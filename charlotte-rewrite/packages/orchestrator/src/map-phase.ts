import type { Chunk } from "@charlotte/native-core";
import { generateStructured, optional } from "@charlotte/providers";
import type {
  GenerateStructuredOptions,
  RequestLimiter,
  RequestUsage,
} from "@charlotte/providers";
import { summaryResponseSchema } from "@charlotte/schemas";
import type { ChunkSummary } from "@charlotte/schemas";

import { mapWithConcurrency } from "./concurrency";
import { buildChunkSummaryPrompt, buildSystemPrompt } from "./prompt";

/** Below this chunk count, map concurrency is just the chunk count (capped
 * at 3); at or above it, Christina's `MAX_CONCURRENT_REQUESTS` applies. */
const SMALL_BATCH_THRESHOLD = 3;
const MAX_CONCURRENT_REQUESTS = 5;

/** Matches Christina's `map_concurrency`. */
export const mapConcurrency = (
  chunkCount: number,
  concurrencyLimit: number
): number => {
  const base =
    chunkCount <= SMALL_BATCH_THRESHOLD
      ? Math.min(chunkCount, SMALL_BATCH_THRESHOLD)
      : MAX_CONCURRENT_REQUESTS;
  return Math.max(1, Math.min(base, concurrencyLimit));
};

/** Builds a readable fallback summary directly from file names when a
 * chunk's model call fails, or returns an empty/unusable summary. Ported
 * from Christina's `fallback_summary_from_files`. */
export const fallbackSummaryFromFiles = (files: readonly string[]): string => {
  if (files.length === 0) {
    return "Update staged files";
  }
  if (files.length === 1) {
    return `Update ${files[0]}`;
  }

  const previewLimit = 3;
  const preview = files.slice(0, previewLimit).join(", ");
  return files.length > previewLimit
    ? `Update ${files.length} files: ${preview} …`
    : `Update ${files.length} files: ${preview}`;
};

export interface MapPhaseOptions {
  readonly model: GenerateStructuredOptions<unknown>["model"];
  readonly concurrencyLimit: number;
  readonly maxPartialFailureRate: number;
  readonly limiter: RequestLimiter;
  readonly signal?: AbortSignal;
}

export interface MapPhaseResult {
  readonly summaries: ChunkSummary[];
  readonly failedChunks: number;
  readonly failedFiles: string[];
  readonly promptTokens: number;
  readonly completionTokens: number;
  readonly requests: RequestUsage[];
}

/**
 * Summarizes every chunk concurrently, tolerating per-chunk failure up to
 * `maxPartialFailureRate`. Ported from Christina's `map_phase`.
 *
 * Simplification: Christina fast-aborts on a detected "systemic" failure
 * (auth/rate-limit errors that would doom every remaining call) before the
 * failure-rate threshold is even reached. This port only checks the
 * failure-rate threshold and the all-failed case; a systemic-failure
 * fast-path can be added once `@charlotte/providers` exposes an error
 * classification to check against.
 */
export const mapPhase = async (
  chunks: readonly Chunk[],
  options: MapPhaseOptions
): Promise<MapPhaseResult> => {
  options.signal?.throwIfAborted();

  const concurrency = mapConcurrency(chunks.length, options.concurrencyLimit);
  let promptTokens = 0;
  let completionTokens = 0;
  const requests: RequestUsage[] = [];

  type ChunkOutcome =
    | { ok: true; summary: ChunkSummary }
    | { ok: false; files: readonly string[] };

  const outcomes = await mapWithConcurrency(
    chunks,
    concurrency,
    async (chunk): Promise<ChunkOutcome> => {
      options.signal?.throwIfAborted();
      try {
        const prompt = `${buildSystemPrompt()}\n\n${buildChunkSummaryPrompt(chunk.content)}`;
        const result = await generateStructured({
          limiter: options.limiter,
          model: options.model,
          prompt,
          schema: summaryResponseSchema,
          ...optional("signal", options.signal),
        });
        promptTokens += result.promptTokens;
        completionTokens += result.completionTokens;
        requests.push(result.usage);

        const summary = result.object.summary.trim();
        return {
          ok: true,
          summary: {
            files: [...chunk.filePaths],
            summary:
              summary.length > 0
                ? summary
                : fallbackSummaryFromFiles(chunk.filePaths),
          },
        };
      } catch {
        return { files: chunk.filePaths, ok: false };
      }
    }
  );

  const summaries: ChunkSummary[] = [];
  const failedFiles: string[] = [];
  let failedChunks = 0;

  for (const outcome of outcomes) {
    if (outcome.ok) {
      summaries.push(outcome.summary);
    } else {
      failedChunks += 1;
      failedFiles.push(...outcome.files);
    }
  }

  if (summaries.length === 0) {
    throw new Error(
      `All ${chunks.length} chunks failed to process. Files affected: ${failedFiles.join(", ")}`
    );
  }

  const totalChunks = summaries.length + failedChunks;
  const failureRate = failedChunks / totalChunks;
  if (failureRate > options.maxPartialFailureRate) {
    throw new Error(
      `Partial failure rate too high: ${failedChunks}/${totalChunks} chunks failed (${Math.round(failureRate * 100)}%). This exceeds the ${Math.round(options.maxPartialFailureRate * 100)}% threshold. Files affected: ${failedFiles.join(", ")}`
    );
  }

  return {
    completionTokens,
    failedChunks,
    failedFiles,
    promptTokens,
    requests,
    summaries,
  };
};
