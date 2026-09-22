import type { CommitValidationMode } from "@charlotte/config";
import { generateStructured, type GenerateStructuredOptions } from "@charlotte/providers";
import type { Chunk } from "@charlotte/native-core";
import { commitResponseSchema, type Warning } from "@charlotte/schemas";
import { validateOrSalvage } from "./commit-message";
import { detectContradictions, extractIntent, fallbackThemesFromSummaries, type IntentResult } from "./intent";
import { mapPhase } from "./map-phase";
import { buildDirectPrompt, buildSystemPrompt, type PromptContext } from "./prompt";
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
}

async function directGeneration(chunk: Chunk, options: GenerateCommitMessageOptions): Promise<GenerationResult> {
  options.signal?.throwIfAborted();

  const prompt = `${buildSystemPrompt()}\n\n${buildDirectPrompt(chunk.content, options.context)}`;
  const result = await generateStructured({ model: options.model, schema: commitResponseSchema, prompt, signal: options.signal });

  const cleaned = cleanResponse(result.object.message);
  const validated = validateOrSalvage(cleaned, options.validationMode, options.maxLength);

  return {
    message: validated.message,
    truncated: false,
    salvaged: validated.salvaged,
    failedChunks: 0,
    failedFiles: [],
    totalChunks: 1,
    warnings: [],
  };
}

/**
 * Generates a Conventional Commit message from already-chunked diff
 * content. Ported from Christina's
 * `generate_commit_message_with_trace_and_cancellation`: an empty chunk
 * list is a caller error, a single chunk skips straight to direct
 * generation, and everything else runs map → (intent) → reduce.
 */
export async function generateCommitMessage(chunks: readonly Chunk[], options: GenerateCommitMessageOptions): Promise<GenerationResult> {
  options.signal?.throwIfAborted();

  if (chunks.length === 0) {
    throw new Error("No diff chunks to process");
  }

  if (chunks.length === 1) {
    return directGeneration(chunks[0] as Chunk, options);
  }

  const mapResult = await mapPhase(chunks, {
    model: options.model,
    concurrencyLimit: options.concurrencyLimit,
    maxPartialFailureRate: options.maxPartialFailureRate,
    signal: options.signal,
  });
  options.signal?.throwIfAborted();

  let intentResult: IntentResult;
  if (mapResult.summaries.length <= MAX_SUMMARIES_WITHOUT_INTENT) {
    const warnings = detectContradictions(mapResult.summaries);
    intentResult = { themes: fallbackThemesFromSummaries(mapResult.summaries), fallbackUsed: false, warnings, promptTokens: 0, completionTokens: 0 };
  } else {
    intentResult = await extractIntent(mapResult.summaries, {
      model: options.model,
      concurrencyLimit: options.concurrencyLimit,
      signal: options.signal,
    });
  }
  options.signal?.throwIfAborted();

  const reduceResult = await reducePhase(intentResult.themes, {
    model: options.model,
    context: options.context,
    validationMode: options.validationMode,
    maxLength: options.maxLength,
    signal: options.signal,
  });

  return {
    message: reduceResult.message,
    truncated: false,
    salvaged: reduceResult.salvaged,
    failedChunks: mapResult.failedChunks,
    failedFiles: mapResult.failedFiles,
    totalChunks: chunks.length,
    warnings: intentResult.warnings,
  };
}
