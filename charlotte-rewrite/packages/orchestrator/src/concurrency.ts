/**
 * Runs `fn` over `items` with at most `limit` calls in flight at once,
 * replacing Christina's semaphore-bounded `buffer_unordered` stream
 * (`christina/src/orchestrator/mod.rs`'s `map_phase`) with a plain
 * `Promise`-based limiter, per `07-orchestrator-pipeline.md`.
 */
export const mapWithConcurrency = async <T, R>(
  items: readonly T[],
  limit: number,
  fn: (item: T, index: number) => Promise<R>
): Promise<R[]> => {
  const results: R[] = Array.from({ length: items.length });
  let nextIndex = 0;

  // Each worker is itself the concurrency limit: it must finish one item
  // before pulling the next, since running every item's `fn` call at once
  // is exactly what `limit` exists to prevent.
  // oxlint-disable no-await-in-loop
  const worker = async (): Promise<void> => {
    for (;;) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) {
        return;
      }
      const item = items[index] as T;
      results[index] = await fn(item, index);
    }
  };
  // oxlint-enable no-await-in-loop

  const workerCount = Math.max(1, Math.min(limit, items.length));
  await Promise.all(Array.from({ length: workerCount }, worker));

  return results;
};
