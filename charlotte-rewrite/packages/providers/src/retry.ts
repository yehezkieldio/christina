import { setTimeout as delay } from "node:timers/promises";

/**
 * Exponential backoff with full jitter, ported from Christina's
 * `christina/src/orchestrator/retry.rs`. Full jitter (delay uniformly
 * sampled from `[0, base * 2^attempt]`, not a fixed exponential value)
 * spreads out retries after a simultaneous failure instead of having every
 * caller retry at exactly the same instants.
 */
export interface RetryPolicy {
  readonly maxRetries: number;
  readonly baseDelayMs: number;
  readonly withJitter: boolean;
}

export interface RetryPolicyOptions {
  readonly maxRetries?: number;
  readonly baseDelayMs?: number;
  readonly withJitter?: boolean;
}

export const createRetryPolicy = (options: RetryPolicyOptions = {}): RetryPolicy => ({
  baseDelayMs: options.baseDelayMs ?? 1000,
  maxRetries: options.maxRetries ?? 3,
  withJitter: options.withJitter ?? true,
});

/** A source of numbers in `[0, 1)`. Defaults to `Math.random`; tests pass a
 * seeded generator so retry-delay assertions are deterministic, matching
 * Christina's `calculate_delay_with_seed` pattern. */
export type RandomSource = () => number;

export const calculateDelayMs = (
  policy: RetryPolicy,
  attempt: number,
  random: RandomSource = Math.random
): number => {
  const maxDelayMs = policy.baseDelayMs * 2 ** attempt;
  if (!policy.withJitter) {
    return maxDelayMs;
  }
  return Math.floor(random() * (maxDelayMs + 1));
};

export const sleep = (ms: number, signal?: AbortSignal): Promise<void> => {
  if (ms <= 0) {
    signal?.throwIfAborted();
    return Promise.resolve();
  }
  // `node:timers/promises`'s `setTimeout` accepts an abort signal natively,
  // rejecting with the signal's reason when aborted — no need to hand-roll
  // a `new Promise` plus a manual "abort" listener and `clearTimeout`.
  return delay(ms, undefined, { signal });
};

export type IsTransient<E> = (error: E) => boolean;

export interface RetryOptions<E> {
  readonly isTransient: IsTransient<E>;
  readonly random?: RandomSource;
  readonly signal?: AbortSignal;
}

/** Retries `operation` under `policy` until it succeeds, a non-transient
 * error is thrown, or `policy.maxRetries` is exhausted. */
export const retryWithBackoff = async <T, E = unknown>(
  policy: RetryPolicy,
  operation: () => Promise<T>,
  options: RetryOptions<E>
): Promise<T> => {
  let attempt = 0;
  // Each retry depends on the previous attempt's outcome — there is nothing
  // to run concurrently, `operation` must finish (or fail) before deciding
  // whether to retry it.
  // oxlint-disable no-await-in-loop
  for (;;) {
    options.signal?.throwIfAborted();
    try {
      return await operation();
    } catch (error) {
      if (!options.isTransient(error as E) || attempt >= policy.maxRetries) {
        throw error;
      }
      await sleep(
        calculateDelayMs(policy, attempt, options.random),
        options.signal
      );
      attempt += 1;
    }
  }
  // oxlint-enable no-await-in-loop
};
