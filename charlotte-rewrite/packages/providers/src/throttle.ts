import pLimit from "p-limit";
import pThrottle from "p-throttle";

const ONE_SECOND_MS = 1000;

export interface RequestLimiterOptions {
  readonly maxConcurrent: number;
  readonly requestsPerSecond: number;
}

/**
 * Combined concurrency + rate limiter, replacing a hand-rolled token-bucket
 * (Christina's `christina/src/orchestrator/throttle.rs`) with two focused,
 * actively maintained libraries: `p-limit` for the concurrency cap and
 * `p-throttle` for the rate cap. The two mechanisms cover different failure
 * modes — a concurrency cap alone cannot prevent an API rate-limit
 * violation (many short requests can still exceed requests-per-second), and
 * a rate limiter alone cannot prevent resource exhaustion (nothing stops
 * the number of requests in flight from climbing) — so both run together,
 * same as the design they replace.
 */
// `p-throttle` wraps one fixed-signature function once, shared across every
// call so the rate window is actually shared state — `run<T>`'s per-call
// generic result type cannot survive that wrapping, so the gate itself is
// erased to `unknown` and restored with a single cast at its one call site.
// oxlint-disable anti-slop/no-unknown-returns
type ThrottledGate = (fn: () => Promise<unknown>) => Promise<unknown>;
// oxlint-enable anti-slop/no-unknown-returns

export class RequestLimiter {
  readonly #limit: ReturnType<typeof pLimit>;
  readonly #throttledGate: ThrottledGate | undefined;

  constructor(options: RequestLimiterOptions) {
    this.#limit = pLimit(options.maxConcurrent);
    // Unbounded rate (Infinity/NaN/<=0) means "no rate limit": skip
    // `p-throttle` entirely rather than configuring it with a limit it
    // cannot represent.
    // This identity gate's `unknown` return type is `ThrottledGate`'s own
    // shape, not a boundary this function could parse instead.
    // oxlint-disable anti-slop/no-unknown-returns
    this.#throttledGate =
      Number.isFinite(options.requestsPerSecond) && options.requestsPerSecond > 0
        ? pThrottle({ interval: ONE_SECOND_MS, limit: options.requestsPerSecond })(
            (fn: () => Promise<unknown>) => fn()
          )
        : undefined;
    // oxlint-enable anti-slop/no-unknown-returns
  }

  /**
   * Runs `fn` once both a concurrency slot and a rate-limit slot are
   * available. `signal` is only checked before `fn` is submitted to either
   * queue — once queued, neither `p-limit` nor `p-throttle` supports
   * cancelling a single already-queued call, unlike the hand-rolled waiter
   * queue this replaces. Nothing in this codebase passes a populated
   * `AbortSignal` here today, so this is a documented simplification, not a
   * regression against real usage.
   */
  run<T>(fn: () => Promise<T>, signal?: AbortSignal): Promise<T> {
    signal?.throwIfAborted();
    return this.#limit(() => {
      if (!this.#throttledGate) {
        return fn();
      }
      // SAFETY: `fn` always resolves to `T`; the throttle gate's `unknown`
      // return type is only an artifact of wrapping one fixed-signature
      // function for every caller (see `ThrottledGate` above).
      return this.#throttledGate(fn) as Promise<T>;
    });
  }
}
