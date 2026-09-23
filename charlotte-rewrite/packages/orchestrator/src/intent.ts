import { generateStructured, optional } from "@charlotte/providers";
import type { GenerateStructuredOptions } from "@charlotte/providers";
import { themeResponseSchema } from "@charlotte/schemas";
import type {
  ChunkSummary,
  SubTheme,
  ThemeItem,
  Warning,
} from "@charlotte/schemas";

import { mapWithConcurrency } from "./concurrency";
import { buildIntentPrompt, buildSystemPrompt } from "./prompt";

/** Matches Christina's `MAX_SUMMARIES_PER_INTENT_BATCH`: above this count,
 * intent extraction batches summaries and aggregates per-batch sub-themes
 * instead of sending everything in one call. */
export const MAX_SUMMARIES_PER_INTENT_BATCH = 20;

const CONTRADICTORY_PAIRS: readonly (readonly [string, string])[] = [
  ["add", "remove"],
  ["create", "delete"],
  ["implement", "remove"],
  ["introduce", "delete"],
  ["new", "delete"],
];

/** Flags a summary batch that mentions both halves of a known
 * action/counteraction pair (e.g. "add" and "remove"), a heuristic signal
 * that two chunks may describe conflicting changes. Pure, no model call.
 * Ported from Christina's `detect_contradictions`, which only logs; this
 * port returns structured `Warning` data instead (per the open decision in
 * `07-orchestrator-pipeline.md`). Stops at the first match, matching
 * Christina's early return. */
export const detectContradictions = (
  summaries: readonly ChunkSummary[]
): Warning[] => {
  const text = summaries.map((s) => s.summary.toLowerCase());

  for (const [action, counteraction] of CONTRADICTORY_PAIRS) {
    const hasAction = text.some((s) => s.includes(action));
    const hasCounteraction = text.some((s) => s.includes(counteraction));
    if (hasAction && hasCounteraction) {
      return [{ action, counteraction, kind: "contradiction" }];
    }
  }

  return [];
};

const combinedDescription = (summaries: readonly ChunkSummary[]): string => {
  const parts = summaries
    .map((s) => s.summary.trim())
    .filter((s) => s.length > 0);
  return parts.length > 0 ? parts.join("; ") : "Code changes";
};

/** Ported from Christina's `fallback_themes_from_summaries`: used when
 * intent extraction is skipped (small summary count) or fails outright. */
export const fallbackThemesFromSummaries = (
  summaries: readonly ChunkSummary[]
): ThemeItem[] => {
  const totalFiles = summaries.reduce((sum, s) => sum + s.files.length, 0);
  return [
    {
      description: combinedDescription(summaries),
      fileCount: totalFiles,
      scope: null,
      title: "Code changes",
    },
  ];
};

/** Ported from Christina's `fallback_sub_themes_from_summaries`: used when
 * one batch's sub-theme extraction call fails during hierarchical intent
 * extraction. */
export const fallbackSubThemesFromSummaries = (
  batch: readonly ChunkSummary[]
): SubTheme[] => {
  const totalFiles = batch.reduce((sum, s) => sum + s.files.length, 0);
  return [
    {
      description: combinedDescription(batch),
      fileCount: totalFiles,
      scope: null,
      title: "Code changes",
    },
  ];
};

/** Groups sub-themes by scope, picks the most common title per group
 * (Christina breaks ties by whichever title the traversal order favors;
 * this port keeps the first-seen title among ties for the same reason:
 * neither language guarantees a stable "true" majority winner here),
 * joins descriptions, sums file counts, then keeps the 3 largest groups by
 * file count. Ported from Christina's `aggregate_sub_themes`. */
export const aggregateSubThemes = (
  subThemes: readonly SubTheme[]
): ThemeItem[] => {
  const groups = new Map<string | null, SubTheme[]>();
  for (const theme of subThemes) {
    const group = groups.get(theme.scope);
    if (group) {
      group.push(theme);
    } else {
      groups.set(theme.scope, [theme]);
    }
  }

  const merged: ThemeItem[] = [];
  for (const [scope, themes] of groups) {
    const totalFiles = themes.reduce((sum, t) => sum + t.fileCount, 0);

    let title: string;
    if (themes.length === 1) {
      title = themes[0]?.title ?? "Code changes";
    } else {
      const counts = new Map<string, number>();
      for (const theme of themes) {
        counts.set(theme.title, (counts.get(theme.title) ?? 0) + 1);
      }
      let best: string | undefined;
      let bestCount = 0;
      for (const [candidate, count] of counts) {
        if (count > bestCount) {
          best = candidate;
          bestCount = count;
        }
      }
      title = best ?? "Code changes";
    }

    const description = themes.map((t) => t.description).join("; ");
    merged.push({ description, fileCount: totalFiles, scope, title });
  }

  merged.sort((a, b) => b.fileCount - a.fileCount);
  return merged.slice(0, 3);
};

export interface IntentOptions {
  readonly model: GenerateStructuredOptions<unknown>["model"];
  readonly concurrencyLimit: number;
  readonly signal?: AbortSignal;
}

export interface IntentResult {
  readonly themes: ThemeItem[];
  readonly fallbackUsed: boolean;
  readonly warnings: Warning[];
  readonly promptTokens: number;
  readonly completionTokens: number;
}

const extractSubThemes = async (
  batch: readonly ChunkSummary[],
  options: IntentOptions
): Promise<{
  themes: SubTheme[];
  promptTokens: number;
  completionTokens: number;
}> => {
  options.signal?.throwIfAborted();
  const prompt = `${buildSystemPrompt()}\n\n${buildIntentPrompt(batch)}`;
  const result = await generateStructured({
    model: options.model,
    prompt,
    schema: themeResponseSchema,
    ...optional("signal", options.signal),
  });

  const themes: SubTheme[] = result.object.themes
    .filter((t) => t.title.trim().length > 0 && t.description.trim().length > 0)
    .map((t) => ({
      description: t.description.trim(),
      fileCount: t.fileCount,
      scope: t.scope,
      title: t.title.trim(),
    }));

  if (themes.length === 0) {
    throw new Error("No valid sub-themes found in response");
  }

  return {
    completionTokens: result.completionTokens,
    promptTokens: result.promptTokens,
    themes,
  };
};

/** Ported from Christina's `extract_intent_hierarchical`: batches
 * summaries at `MAX_SUMMARIES_PER_INTENT_BATCH`, extracts sub-themes per
 * batch concurrently (falling back per-batch on failure), then aggregates
 * every sub-theme into the final theme list. */
const extractIntentHierarchical = async (
  summaries: readonly ChunkSummary[],
  options: IntentOptions
): Promise<IntentResult> => {
  const batches: ChunkSummary[][] = [];
  for (let i = 0; i < summaries.length; i += MAX_SUMMARIES_PER_INTENT_BATCH) {
    batches.push(summaries.slice(i, i + MAX_SUMMARIES_PER_INTENT_BATCH));
  }

  let promptTokens = 0;
  let completionTokens = 0;
  let anyFallback = false;

  const batchResults = await mapWithConcurrency(
    batches,
    Math.max(1, Math.min(options.concurrencyLimit, batches.length)),
    async (batch) => {
      options.signal?.throwIfAborted();
      try {
        const result = await extractSubThemes(batch, options);
        promptTokens += result.promptTokens;
        completionTokens += result.completionTokens;
        return result.themes;
      } catch {
        anyFallback = true;
        return fallbackSubThemesFromSummaries(batch);
      }
    }
  );

  const allSubThemes = batchResults.flat();
  if (allSubThemes.length === 0) {
    return {
      completionTokens,
      fallbackUsed: true,
      promptTokens,
      themes: fallbackThemesFromSummaries(summaries),
      warnings: [],
    };
  }

  const themes = aggregateSubThemes(allSubThemes);
  return {
    completionTokens,
    fallbackUsed: anyFallback,
    promptTokens,
    themes,
    warnings: [],
  };
};

/** Ported from Christina's `extract_intent`: always checks for
 * contradictions first, then either delegates to hierarchical batching
 * (above `MAX_SUMMARIES_PER_INTENT_BATCH`) or makes a single themed-batch
 * call, falling back to `fallbackThemesFromSummaries` on any failure. */
export const extractIntent = async (
  summaries: readonly ChunkSummary[],
  options: IntentOptions
): Promise<IntentResult> => {
  options.signal?.throwIfAborted();
  const warnings = detectContradictions(summaries);

  if (summaries.length > MAX_SUMMARIES_PER_INTENT_BATCH) {
    const result = await extractIntentHierarchical(summaries, options);
    return { ...result, warnings: [...warnings, ...result.warnings] };
  }

  try {
    const prompt = `${buildSystemPrompt()}\n\n${buildIntentPrompt(summaries)}`;
    const result = await generateStructured({
      model: options.model,
      prompt,
      schema: themeResponseSchema,
      ...optional("signal", options.signal),
    });

    const themes: ThemeItem[] = result.object.themes
      .filter(
        (t) => t.title.trim().length > 0 && t.description.trim().length > 0
      )
      .map((t) => ({
        description: t.description.trim(),
        fileCount: t.fileCount,
        scope: t.scope,
        title: t.title.trim(),
      }));

    if (themes.length === 0) {
      throw new Error("No valid themes found in response");
    }

    return {
      completionTokens: result.completionTokens,
      fallbackUsed: false,
      promptTokens: result.promptTokens,
      themes,
      warnings,
    };
  } catch {
    return {
      completionTokens: 0,
      fallbackUsed: true,
      promptTokens: 0,
      themes: fallbackThemesFromSummaries(summaries),
      warnings,
    };
  }
};
