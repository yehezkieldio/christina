import { randomUUID } from "node:crypto";

import { APICallError, generateObject } from "ai";
import type { LanguageModel } from "ai";
import type { z } from "zod";

import { optional } from "./optional";
import { createRetryPolicy, retryWithBackoff } from "./retry";
import type { RequestLimiter } from "./throttle";

export interface GenerateStructuredOptions<T> {
  readonly model: LanguageModel;
  readonly schema: z.ZodType<T>;
  readonly prompt: string;
  readonly limiter: RequestLimiter;
  readonly signal?: AbortSignal;
}

/**
 * One model call's usage, in the shape `08-session-storage-and-stats.md`'s
 * `request`/`response` transcript events need. Every caller of
 * `generateStructured` collects these instead of re-deriving model/provider
 * identity or re-timing the call itself.
 */
export interface RequestUsage {
  readonly requestId: string;
  readonly provider: string;
  readonly model: string;
  readonly promptTokens: number;
  readonly completionTokens: number;
  /** Prompt tokens served from the provider's cache (e.g. Anthropic's
   * `cache_read_input_tokens`): billed far below a fresh input token. */
  readonly cacheReadTokens: number;
  /** Prompt tokens written into the provider's cache on this call (e.g.
   * Anthropic's `cache_creation_input_tokens`): billed above a fresh input
   * token. Distinct from `cacheReadTokens` — collapsing the two into one
   * count is exactly the mistake that makes a later cost pass wrong, since
   * writes and reads price in opposite directions. */
  readonly cacheWriteTokens: number;
  readonly startedAt: string;
  readonly latencyMs: number;
}

export interface GenerateStructuredResult<T> {
  readonly object: T;
  readonly promptTokens: number;
  readonly completionTokens: number;
  readonly usage: RequestUsage;
}

/** `LanguageModel`'s two shapes come from the AI SDK's own type (a bare
 * model-id string, the AI Gateway shorthand, or a real provider object), not
 * from untrusted external input, so there is no schema to parse this
 * against instead. `@charlotte/providers` never constructs the string
 * variant — `resolveModel` always returns a real `LanguageModelV2`/`V3`/`V4`
 * object — but the type doesn't know that, so this narrows at the one call
 * site that needs `provider`/`modelId` off of it. */
// oxlint-disable anti-slop/no-runtime-typeof
const modelIdentity = (
  model: LanguageModel
): { provider: string; model: string } =>
  typeof model === "string"
    ? { model, provider: "unknown" }
    : { model: model.modelId, provider: model.provider };
// oxlint-enable anti-slop/no-runtime-typeof

// `isTransient` classifies whatever `generateObject` can throw, which is not
// bounded by a schema — `unknown` is the honest type for a catch-clause
// classifier, not a shortcut around one.
// oxlint-disable-next-line anti-slop/no-unknown-parameters
const isTransient = (error: unknown): boolean =>
  error instanceof APICallError && error.isRetryable;

const defaultRetryPolicy = createRetryPolicy();

/**
 * Wraps the AI SDK's `generateObject` with retry (`retry.ts`) and
 * concurrency/rate throttling (`throttle.ts`). This is the one function
 * `@charlotte/orchestrator` calls for every structured model request, per
 * `06-providers-and-ai-sdk.md` and `07-orchestrator-pipeline.md`.
 *
 * `options.limiter` is caller-supplied rather than a module-level default:
 * a shared singleton with hardcoded bounds cannot reflect a user's
 * configured `maxConcurrentRequests`/`requestsPerSecond`, so the caller
 * (`@charlotte/orchestrator`) builds one `RequestLimiter` per run from
 * `ResolvedConfig` and threads it through every call in that run.
 */
export const generateStructured = <T>(
  options: GenerateStructuredOptions<T>
): Promise<GenerateStructuredResult<T>> =>
  options.limiter.run(
    () =>
      retryWithBackoff(
        defaultRetryPolicy,
        async () => {
          const startedAt = new Date();
          const startTime = performance.now();
          const result = await generateObject({
            model: options.model,
            prompt: options.prompt,
            schema: options.schema,
            ...optional("abortSignal", options.signal),
          });
          const promptTokens = result.usage.inputTokens ?? 0;
          const completionTokens = result.usage.outputTokens ?? 0;
          return {
            completionTokens,
            object: result.object,
            promptTokens,
            usage: {
              ...modelIdentity(options.model),
              cacheReadTokens:
                result.usage.inputTokenDetails.cacheReadTokens ?? 0,
              cacheWriteTokens:
                result.usage.inputTokenDetails.cacheWriteTokens ?? 0,
              completionTokens,
              latencyMs: performance.now() - startTime,
              promptTokens,
              requestId: randomUUID(),
              startedAt: startedAt.toISOString(),
            },
          };
        },
        { isTransient, ...optional("signal", options.signal) }
      ),
    options.signal
  );
