import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  add,
  chunkDiff,
  countTokens,
  isBinaryContent,
  readCommitHistory,
  readStagedDiff,
} from "./index";

test("add calls the native core through bun:ffi", () => {
  expect(add(2, 3)).toBe(5);
});

test("countTokens returns a positive count for non-empty text", () => {
  expect(countTokens("hello world")).toBeGreaterThan(0);
});

test("countTokens returns zero for empty text", () => {
  expect(countTokens("")).toBe(0);
});

test("isBinaryContent detects a NUL byte", () => {
  const bytes = new Uint8Array([0x00, 0x01, 0x02]);
  expect(isBinaryContent(bytes, "file.dat")).toBe(true);
});

test("isBinaryContent detects a known binary extension with clean bytes", () => {
  const bytes = new TextEncoder().encode("not binary");
  expect(isBinaryContent(bytes, "image.png")).toBe(true);
});

test("isBinaryContent is false for clean text and a text extension", () => {
  const bytes = new TextEncoder().encode("fn main() {}\n");
  expect(isBinaryContent(bytes, "main.rs")).toBe(false);
});

test("chunkDiff returns a single chunk for a small diff", () => {
  const diff = "diff --git a/file.txt b/file.txt\n@@ -0,0 +1 @@\n+hello\n";
  const chunks = chunkDiff(diff, 10_000, 100);
  expect(chunks.length).toBe(1);
  expect(chunks[0]?.filePaths).toEqual(["file.txt"]);
});

test("chunkDiff returns nothing for an empty diff", () => {
  expect(chunkDiff("", 1000, 100)).toEqual([]);
});

const initRepo = (): string => {
  const dir = mkdtempSync(path.join(tmpdir(), "charlotte-native-core-"));
  const run = (...args: string[]) =>
    Bun.spawnSync({ cmd: ["git", ...args], cwd: dir });
  run("init", "-q");
  run("config", "user.name", "Test User");
  run("config", "user.email", "test@example.com");
  return dir;
};

test("readStagedDiff returns the staged diff and file list", () => {
  const dir = initRepo();
  try {
    writeFileSync(path.join(dir, "file.txt"), "hello\n");
    Bun.spawnSync({ cmd: ["git", "add", "file.txt"], cwd: dir });

    const staged = readStagedDiff(dir);
    expect(staged.files).toEqual(["file.txt"]);
    expect(staged.diff).toContain("+hello");
  } finally {
    rmSync(dir, { force: true, recursive: true });
  }
});

test("readStagedDiff on an empty index returns an empty result", () => {
  const dir = initRepo();
  try {
    const staged = readStagedDiff(dir);
    expect(staged.files).toEqual([]);
    expect(staged.diff).toBe("");
  } finally {
    rmSync(dir, { force: true, recursive: true });
  }
});

test("readCommitHistory walks recent commit subjects", () => {
  const dir = initRepo();
  try {
    writeFileSync(path.join(dir, "a.txt"), "a\n");
    Bun.spawnSync({ cmd: ["git", "add", "a.txt"], cwd: dir });
    Bun.spawnSync({
      cmd: ["git", "commit", "-q", "-m", "first commit"],
      cwd: dir,
    });
    writeFileSync(path.join(dir, "b.txt"), "b\n");
    Bun.spawnSync({ cmd: ["git", "add", "b.txt"], cwd: dir });
    Bun.spawnSync({
      cmd: ["git", "commit", "-q", "-m", "second commit"],
      cwd: dir,
    });

    const history = readCommitHistory(dir, 5);
    expect(history.length).toBe(2);
    expect(history[0]?.subject).toBe("second commit");
    expect(history[1]?.subject).toBe("first commit");
  } finally {
    rmSync(dir, { force: true, recursive: true });
  }
});
