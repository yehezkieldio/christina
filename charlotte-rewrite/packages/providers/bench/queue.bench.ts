/**
 * Standalone measurement, not part of the shipped package: settles whether
 * `RequestLimiter`'s `Array.prototype.shift()`-based waiter queue
 * (`../src/throttle.ts`) should become a ring buffer, by measuring both at
 * queue depths realistic for this codebase (bounded by diff-chunk count,
 * so single digits to low tens) and at depths large enough to show where
 * `shift()`'s O(n) reindexing actually starts to cost something.
 *
 * Run with: bun run packages/providers/bench/queue.bench.ts
 */

// This file compares exactly the two candidate queue implementations named
// in the header comment side by side; splitting them into separate files
// would only make the comparison harder to read.
// oxlint-disable max-classes-per-file
interface Queue<T> {
  push: (value: T) => void;
  shift: () => T | undefined;
  get length(): number;
}

class ArrayQueue<T> implements Queue<T> {
  #items: T[] = [];
  push(value: T): void {
    this.#items.push(value);
  }
  shift(): T | undefined {
    return this.#items.shift();
  }
  get length(): number {
    return this.#items.length;
  }
}

/** Circular buffer: push/shift are both O(1) amortized, growing (doubling)
 * only when full instead of reindexing on every dequeue. */
class RingQueue<T> implements Queue<T> {
  #buffer: (T | undefined)[];
  #head = 0;
  #size = 0;

  constructor(initialCapacity = 16) {
    this.#buffer = Array.from({ length: initialCapacity });
  }

  push(value: T): void {
    if (this.#size === this.#buffer.length) {
      this.#grow();
    }
    this.#buffer[(this.#head + this.#size) % this.#buffer.length] = value;
    this.#size += 1;
  }

  shift(): T | undefined {
    if (this.#size === 0) {
      return undefined;
    }
    const value = this.#buffer[this.#head];
    this.#buffer[this.#head] = undefined;
    this.#head = (this.#head + 1) % this.#buffer.length;
    this.#size -= 1;
    return value;
  }

  get length(): number {
    return this.#size;
  }

  #grow(): void {
    const next: (T | undefined)[] = Array.from({
      length: this.#buffer.length * 2,
    });
    for (let i = 0; i < this.#size; i += 1) {
      next[i] = this.#buffer[(this.#head + i) % this.#buffer.length];
    }
    this.#buffer = next;
    this.#head = 0;
  }
}
// oxlint-enable max-classes-per-file

/** Mirrors `RequestLimiter`'s real access pattern: push `depth` waiters
 * (build-up under contention), then interleave one push with one shift
 * `iterations` times (steady-state: a release immediately followed by the
 * next acquire), which is the actual pattern under sustained concurrency
 * pressure rather than a one-shot drain. */
const runPattern = (
  queue: Queue<number>,
  depth: number,
  iterations: number
): number => {
  for (let i = 0; i < depth; i += 1) {
    queue.push(i);
  }
  let sink = 0;
  for (let i = 0; i < iterations; i += 1) {
    queue.push(i);
    sink += queue.shift() ?? 0;
  }
  return sink;
};

const bench = (label: string, fn: () => void, runs = 5): number => {
  // Warm up the JIT before timing, discard this run.
  fn();
  const samples: number[] = [];
  for (let i = 0; i < runs; i += 1) {
    const start = performance.now();
    fn();
    samples.push(performance.now() - start);
  }
  samples.sort((a, b) => a - b);
  const median = samples[Math.floor(samples.length / 2)] ?? 0;
  console.log(
    `${label}: median ${median.toFixed(3)}ms over ${runs} runs (min ${samples[0]?.toFixed(3)}ms, max ${samples.at(-1)?.toFixed(3)}ms)`
  );
  return median;
};

const DEPTHS = [4, 16, 64, 1000, 10_000, 100_000];
const ITERATIONS = 50_000;

console.log(
  `Steady-state push+shift pattern, ${ITERATIONS} iterations per depth\n`
);

for (const depth of DEPTHS) {
  console.log(`-- queue depth ${depth} --`);
  const arrayMs = bench("  ArrayQueue (shift)", () =>
    runPattern(new ArrayQueue<number>(), depth, ITERATIONS));
  const ringMs = bench("  RingQueue          ", () =>
    runPattern(new RingQueue<number>(), depth, ITERATIONS));
  const factor = arrayMs / ringMs;
  console.log(`  ArrayQueue/RingQueue ratio: ${factor.toFixed(2)}x\n`);
}
