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

export interface GenerateStructuredResult<T> {
  readonly object: T;
  readonly promptTokens: number;
  readonly completionTokens: number;
}

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
          const result = await generateObject({
            model: options.model,
            prompt: options.prompt,
            schema: options.schema,
            ...optional("abortSignal", options.signal),
          });
          return {
            completionTokens: result.usage.outputTokens ?? 0,
            object: result.object,
            promptTokens: result.usage.inputTokens ?? 0,
          };
        },
        { isTransient, ...optional("signal", options.signal) }
      ),
    options.signal
  );
