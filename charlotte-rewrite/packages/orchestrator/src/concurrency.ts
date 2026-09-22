/**
 * Runs `fn` over `items` with at most `limit` calls in flight at once,
 * replacing Christina's semaphore-bounded `buffer_unordered` stream
 * (`christina/src/orchestrator/mod.rs`'s `map_phase`) with a plain
 * `Promise`-based limiter, per `07-orchestrator-pipeline.md`.
 */
export async function mapWithConcurrency<T, R>(
  items: readonly T[],
  limit: number,
  fn: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  const results = new Array<R>(items.length);
  let nextIndex = 0;

  async function worker(): Promise<void> {
    for (;;) {
      const index = nextIndex++;
      if (index >= items.length) {
        return;
      }
      const item = items[index] as T;
      results[index] = await fn(item, index);
    }
  }

  const workerCount = Math.max(1, Math.min(limit, items.length));
  await Promise.all(Array.from({ length: workerCount }, worker));

  return results;
}
