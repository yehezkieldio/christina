import { describe, expect, test } from "bun:test";

import type { ChunkSummary, SubTheme } from "@charlotte/schemas";

import {
  aggregateSubThemes,
  detectContradictions,
  fallbackSubThemesFromSummaries,
  fallbackThemesFromSummaries,
} from "./intent";

const summary = (text: string, files: string[]): ChunkSummary => { files, summary: text };

describe("detectContradictions", () => {
  test("flags an add/remove contradiction", () => {
    const warnings = detectContradictions([
      summary("add the new widget", ["a.ts"]),
      summary("remove the old widget", ["b.ts"]),
    ]);
    expect(warnings).toEqual([
      { action: "add", counteraction: "remove", kind: "contradiction" },
    ]);
  });

  test("returns no warnings for coherent summaries", () => {
    expect(
      detectContradictions([
        summary("add the new widget", ["a.ts"]),
        summary("fix a typo", ["b.ts"]),
      ])
    ).toEqual([]);
  });
});

describe("fallbackThemesFromSummaries", () => {
  test("combines summaries into one theme", () => {
    const themes = fallbackThemesFromSummaries([
      summary("add widget", ["a.ts"]),
      summary("fix typo", ["b.ts", "c.ts"]),
    ]);
    expect(themes).toEqual([
      {
        description: "add widget; fix typo",
        fileCount: 3,
        scope: null,
        title: "Code changes",
      },
    ]);
  });

  test("falls back to a generic description when every summary is blank", () => {
    expect(
      fallbackThemesFromSummaries([summary("  ", [])])[0]?.description
    ).toBe("Code changes");
  });
});

describe("fallbackSubThemesFromSummaries", () => {
  test("combines a batch into one sub-theme", () => {
    const themes = fallbackSubThemesFromSummaries([
      summary("add widget", ["a.ts"]),
    ]);
    expect(themes).toEqual([
      {
        description: "add widget",
        fileCount: 1,
        scope: null,
        title: "Code changes",
      },
    ]);
  });
});

describe("aggregateSubThemes", () => {
  const subTheme = (title: string,
  description: string,
  fileCount: number,
  scope: string | null): SubTheme => { description, fileCount, scope, title };

  test("groups by scope and sums file counts", () => {
    const merged = aggregateSubThemes([
      subTheme("Auth", "add login", 2, "auth"),
      subTheme("Auth", "add logout", 1, "auth"),
      subTheme("UI", "restyle button", 3, "ui"),
    ]);

    expect(merged).toHaveLength(2);
    const auth = merged.find((t) => t.scope === "auth");
    expect(auth?.fileCount).toBe(3);
    expect(auth?.description).toBe("add login; add logout");
  });

  test("keeps only the 3 largest groups by file count", () => {
    const merged = aggregateSubThemes([
      subTheme("A", "a", 1, "a"),
      subTheme("B", "b", 2, "b"),
      subTheme("C", "c", 3, "c"),
      subTheme("D", "d", 4, "d"),
    ]);
    expect(merged).toHaveLength(3);
    expect(merged.map((t) => t.fileCount)).toEqual([4, 3, 2]);
  });
});
