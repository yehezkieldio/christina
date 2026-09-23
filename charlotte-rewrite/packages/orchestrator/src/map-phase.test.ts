import { describe, expect, test } from "bun:test";

import { fallbackSummaryFromFiles, mapConcurrency } from "./map-phase";

describe("fallbackSummaryFromFiles", () => {
  test("handles zero files", () => {
    expect(fallbackSummaryFromFiles([])).toBe("Update staged files");
  });

  test("handles a single file", () => {
    expect(fallbackSummaryFromFiles(["src/main.ts"])).toBe(
      "Update src/main.ts"
    );
  });

  test("previews up to 3 files without an ellipsis", () => {
    expect(fallbackSummaryFromFiles(["a.ts", "b.ts", "c.ts"])).toBe(
      "Update 3 files: a.ts, b.ts, c.ts"
    );
  });

  test("previews the first 3 files with an ellipsis beyond that", () => {
    expect(fallbackSummaryFromFiles(["a.ts", "b.ts", "c.ts", "d.ts"])).toBe(
      "Update 4 files: a.ts, b.ts, c.ts …"
    );
  });
});

describe("mapConcurrency", () => {
  test("caps small batches at the chunk count", () => {
    expect(mapConcurrency(2, 10)).toBe(2);
  });

  test("uses the default concurrent-request bound above the small-batch threshold", () => {
    expect(mapConcurrency(50, 10)).toBe(5);
  });

  test("never exceeds the configured concurrency limit", () => {
    expect(mapConcurrency(50, 2)).toBe(2);
  });

  test("never goes below 1", () => {
    expect(mapConcurrency(0, 0)).toBe(1);
  });
});
