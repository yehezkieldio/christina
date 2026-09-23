import { describe, expect, test } from "bun:test";

import { mapWithConcurrency } from "./concurrency";

describe("mapWithConcurrency", () => {
  test("preserves result order regardless of completion order", async () => {
    const delays = [30, 10, 20, 0];
    const results = await mapWithConcurrency(
      delays,
      4,
      async (delay, index) => {
        await new Promise((resolve) => setTimeout(resolve, delay));
        return index;
      }
    );
    expect(results).toEqual([0, 1, 2, 3]);
  });

  test("never runs more than `limit` callbacks concurrently", async () => {
    let active = 0;
    let maxActive = 0;
    const items = Array.from({ length: 10 }, (_, i) => i);

    await mapWithConcurrency(items, 3, async () => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      await new Promise((resolve) => setTimeout(resolve, 5));
      active -= 1;
    });

    expect(maxActive).toBeLessThanOrEqual(3);
  });

  test("handles an empty input", async () => {
    const results = await mapWithConcurrency([], 4, async (x) => x);
    expect(results).toEqual([]);
  });
});
