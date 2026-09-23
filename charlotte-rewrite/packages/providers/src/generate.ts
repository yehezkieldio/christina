import { APICallError, generateObject } from "ai";
import type { LanguageModel } from "ai";
import type { z } from "zod";

import { optional } from "./optional";
import { createRetryPolicy, retryWithBackoff } from "./retry";
import { RequestLimiter } from "./throttle";

export interface GenerateStructuredOptions<T> {
  readonly model: LanguageModel;
  readonly schema: z.ZodType<T>;
  readonly prompt: string;
  readonly signal?: AbortSignal;
}

export interface GenerateStructuredResult<T> {
  readonly object: T;
  readonly promptTokens: number;
  readonly completionTokens: number;
}

const isTransient = (error: unknown): boolean => error instanceof APICallError && error.isRetryable;

const defaultRetryPolicy = createRetryPolicy();
const defaultLimiter = new RequestLimiter({
  maxConcurrent: 4,
  requestsPerSecond: 5,
});

/**
 * Wraps the AI SDK's `generateObject` with retry (`retry.ts`) and
 * concurrency/rate throttling (`throttle.ts`). This is the one function
 * `@charlotte/orchestrator` calls for every structured model request, per
 * `06-providers-and-ai-sdk.md` and `07-orchestrator-pipeline.md`.
 */
export const generateStructured = async <T>(
  options: GenerateStructuredOptions<T>
): Promise<GenerateStructuredResult<T>> => {
  const release = await defaultLimiter.acquire(options.signal);
  try {
    return await retryWithBackoff(
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
    );
  } finally {
    release();
  }
};
