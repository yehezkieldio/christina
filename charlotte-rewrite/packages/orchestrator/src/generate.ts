import type { CommitValidationMode } from "@charlotte/config";
import type { Chunk } from "@charlotte/native-core";
import { generateStructured, optional } from "@charlotte/providers";
import type {
  GenerateStructuredOptions,
  RequestLimiter,
  RequestUsage,
} from "@charlotte/providers";
import { commitResponseSchema } from "@charlotte/schemas";
import type { Warning } from "@charlotte/schemas";

import { validateOrSalvage } from "./commit-message";
import {
  detectContradictions,
  extractIntent,
  fallbackThemesFromSummaries,
} from "./intent";
import type { IntentResult } from "./intent";
import { mapPhase } from "./map-phase";
import { buildDirectPrompt, buildSystemPrompt } from "./prompt";
import type { PromptContext } from "./prompt";
import { cleanResponse, reducePhase } from "./reduce-phase";

/** Below this summary count, intent extraction is skipped entirely and
 * `fallbackThemesFromSummaries` runs directly — matches Christina's
 * `MAX_SUMMARIES_WITHOUT_INTENT`. */
const MAX_SUMMARIES_WITHOUT_INTENT = 3;

export interface GenerateCommitMessageOptions {
  readonly model: GenerateStructuredOptions<unknown>["model"];
  readonly context?: PromptContext;
  readonly validationMode: CommitValidationMode;
  readonly maxLength?: number;
  readonly concurrencyLimit: number;
  readonly maxPartialFailureRate: number;
  readonly limiter: RequestLimiter;
  readonly signal?: AbortSignal;
}

export interface GenerationResult {
  readonly message: string;
  /** Always `false` at this layer: diff truncation is a
   * `05-diff-processing-and-chunking.md` concern the native core already
   * resolved before chunks reach the orchestrator, not something this
   * pipeline recomputes. Kept on the result shape for parity with
   * Christina's `GenerationResult` and for a future caller that wants to
   * thread native-core truncation flags through. */
  readonly truncated: boolean;
  readonly salvaged: boolean;
  readonly failedChunks: number;
  readonly failedFiles: string[];
  readonly totalChunks: number;
  readonly warnings: Warning[];
  readonly promptTokens: number;
  readonly completionTokens: number;
  readonly requests: RequestUsage[];
}

const directGeneration = async (
  chunk: Chunk,
  options: GenerateCommitMessageOptions
): Promise<GenerationResult> => {
  options.signal?.throwIfAborted();

  const prompt = `${buildSystemPrompt()}\n\n${buildDirectPrompt(chunk.content, options.context)}`;
  const result = await generateStructured({
    limiter: options.limiter,
    model: options.model,
    prompt,
    schema: commitResponseSchema,
    ...optional("signal", options.signal),
  });

  const cleaned = cleanResponse(result.object.message);
  const validated = validateOrSalvage(
    cleaned,
    options.validationMode,
    options.maxLength
  );

  return {
    completionTokens: result.completionTokens,
    failedChunks: 0,
    failedFiles: [],
    message: validated.message,
    promptTokens: result.promptTokens,
    requests: [result.usage],
    salvaged: validated.salvaged,
    totalChunks: 1,
    truncated: false,
    warnings: [],
  };
};

/**
 * Generates a Conventional Commit message from already-chunked diff
 * content. Ported from Christina's
 * `generate_commit_message_with_trace_and_cancellation`: an empty chunk
 * list is a caller error, a single chunk skips straight to direct
 * generation, and everything else runs map → (intent) → reduce.
 */
export const generateCommitMessage = async (
  chunks: readonly Chunk[],
  options: GenerateCommitMessageOptions
): Promise<GenerationResult> => {
  options.signal?.throwIfAborted();

  if (chunks.length === 0) {
    throw new Error("No diff chunks to process");
  }

  if (chunks.length === 1) {
    // SAFETY: `chunks.length === 1` was just checked above.
    return directGeneration(chunks[0] as Chunk, options);
  }

  const mapResult = await mapPhase(chunks, {
    concurrencyLimit: options.concurrencyLimit,
    limiter: options.limiter,
    maxPartialFailureRate: options.maxPartialFailureRate,
    model: options.model,
    ...optional("signal", options.signal),
  });
  options.signal?.throwIfAborted();

  let intentResult: IntentResult;
  if (mapResult.summaries.length <= MAX_SUMMARIES_WITHOUT_INTENT) {
    const warnings = detectContradictions(mapResult.summaries);
    intentResult = {
      completionTokens: 0,
      fallbackUsed: false,
      promptTokens: 0,
      requests: [],
      themes: fallbackThemesFromSummaries(mapResult.summaries),
      warnings,
    };
  } else {
    intentResult = await extractIntent(mapResult.summaries, {
      concurrencyLimit: options.concurrencyLimit,
      limiter: options.limiter,
      model: options.model,
      ...optional("signal", options.signal),
    });
  }
  options.signal?.throwIfAborted();

  const reduceResult = await reducePhase(intentResult.themes, {
    limiter: options.limiter,
    model: options.model,
    validationMode: options.validationMode,
    ...optional("context", options.context),
    ...optional("maxLength", options.maxLength),
    ...optional("signal", options.signal),
  });

  return {
    completionTokens:
      mapResult.completionTokens +
      intentResult.completionTokens +
      reduceResult.completionTokens,
    failedChunks: mapResult.failedChunks,
    failedFiles: mapResult.failedFiles,
    message: reduceResult.message,
    promptTokens:
      mapResult.promptTokens +
      intentResult.promptTokens +
      reduceResult.promptTokens,
    requests: [
      ...mapResult.requests,
      ...intentResult.requests,
      ...reduceResult.requests,
    ],
    salvaged: reduceResult.salvaged,
    totalChunks: chunks.length,
    truncated: false,
    warnings: intentResult.warnings,
  };
};
