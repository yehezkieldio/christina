import { sleep } from "./retry";

/**
 * Combined concurrency + rate limiter, ported from Christina's
 * `christina/src/orchestrator/throttle.rs`. A concurrency cap alone cannot
 * prevent an API rate-limit violation (many short requests can still
 * exceed requests-per-second), and a rate limiter alone cannot prevent
 * resource exhaustion (nothing stops the number of requests in flight from
 * climbing) — the two mechanisms cover different failure modes, so both
 * run together.
 *
 * Token math uses milli-token integers instead of floating point, matching
 * Christina's design: floating-point accumulation would drift over a
 * long-running session, and integer milli-tokens don't.
 */
const ONE_TOKEN_MILLI = 1000;

export interface RequestLimiterOptions {
  readonly maxConcurrent: number;
  readonly requestsPerSecond: number;
}

export class RequestLimiter {
  readonly #maxConcurrent: number;
  #active = 0;
  readonly #waiters: Array<() => void> = [];

  readonly #capacityMilli: number;
  #tokensMilli: number;
  readonly #refillRateMilliPerSec: number;
  #lastRefillMs: number;

  constructor(options: RequestLimiterOptions) {
    this.#maxConcurrent = options.maxConcurrent;

    // Unbounded rate (Infinity/NaN/<=0) means "no rate limit": an
    // effectively infinite bucket that never needs a refill wait.
    this.#refillRateMilliPerSec =
      Number.isFinite(options.requestsPerSecond) && options.requestsPerSecond > 0
        ? Math.ceil(options.requestsPerSecond * ONE_TOKEN_MILLI)
        : Number.POSITIVE_INFINITY;
    // Capacity = 2x the per-second rate, allowing a short burst while still
    // bounding the long-term average rate, matching Christina's choice.
    this.#capacityMilli = Number.isFinite(this.#refillRateMilliPerSec)
      ? this.#refillRateMilliPerSec * 2
      : Number.POSITIVE_INFINITY;
    this.#tokensMilli = this.#capacityMilli;
    this.#lastRefillMs = performance.now();
  }

  /** Waits for both a rate-limit token and a concurrency slot, then returns
   * a release function the caller must call exactly once (typically in a
   * `finally` block) to free the slot for the next waiter. */
  async acquire(signal?: AbortSignal): Promise<() => void> {
    await this.#acquireToken(signal);
    await this.#acquireSlot(signal);
    let released = false;
    return () => {
      if (released) {
        return;
      }
      released = true;
      this.#active -= 1;
      this.#waiters.shift()?.();
    };
  }

  async #acquireToken(signal?: AbortSignal): Promise<void> {
    for (;;) {
      signal?.throwIfAborted();
      const waitMs = this.#tryConsumeToken();
      if (waitMs <= 0) {
        return;
      }
      await sleep(waitMs, signal);
    }
  }

  #tryConsumeToken(): number {
    if (!Number.isFinite(this.#refillRateMilliPerSec)) {
      return 0;
    }
    const now = performance.now();
    const elapsedSec = (now - this.#lastRefillMs) / 1000;
    this.#tokensMilli = Math.min(this.#capacityMilli, this.#tokensMilli + elapsedSec * this.#refillRateMilliPerSec);
    this.#lastRefillMs = now;

    if (this.#tokensMilli >= ONE_TOKEN_MILLI) {
      this.#tokensMilli -= ONE_TOKEN_MILLI;
      return 0;
    }
    const deficitMilli = ONE_TOKEN_MILLI - this.#tokensMilli;
    return Math.ceil((deficitMilli / this.#refillRateMilliPerSec) * 1000);
  }

  #acquireSlot(signal?: AbortSignal): Promise<void> {
    if (this.#active < this.#maxConcurrent) {
      this.#active += 1;
      return Promise.resolve();
    }
    return new Promise((resolve, reject) => {
      const grant = () => {
        signal?.removeEventListener("abort", onAbort);
        this.#active += 1;
        resolve();
      };
      const onAbort = () => {
        const index = this.#waiters.indexOf(grant);
        if (index >= 0) {
          this.#waiters.splice(index, 1);
        }
        reject(signal?.reason);
      };
      signal?.addEventListener("abort", onAbort, { once: true });
      this.#waiters.push(grant);
    });
  }
}
