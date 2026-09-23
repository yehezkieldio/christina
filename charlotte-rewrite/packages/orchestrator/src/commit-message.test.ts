import { describe, expect, test } from "bun:test";

import {
  InvalidCommitMessageError,
  tryExtractValidCommit,
  validateCommitMessage,
  validateOrSalvage,
} from "./commit-message";

describe("validateCommitMessage", () => {
  test("accepts a well-formed conventional commit", () => {
    const result = validateCommitMessage(
      "feat(auth): add OAuth flow",
      "strict"
    );
    expect(result.message).toBe("feat(auth): add OAuth flow");
    expect(result.warnings).toEqual([]);
  });

  test("rejects an empty message", () => {
    expect(() => validateCommitMessage("   ", "strict")).toThrow(
      InvalidCommitMessageError
    );
  });

  test("rejects a message that isn't conventional-commit shaped", () => {
    expect(() => validateCommitMessage("updated some stuff", "strict")).toThrow(
      InvalidCommitMessageError
    );
  });

  test("strict mode rejects an over-length message", () => {
    const long = `feat: ${"x".repeat(100)}`;
    expect(() => validateCommitMessage(long, "strict", 72)).toThrow(
      InvalidCommitMessageError
    );
  });

  test("soft mode warns but allows an over-length message", () => {
    const long = `feat: ${"x".repeat(100)}`;
    const result = validateCommitMessage(long, "soft", 72);
    expect(result.warnings.length).toBe(1);
  });

  test("disabled mode skips the length check", () => {
    const long = `feat: ${"x".repeat(100)}`;
    const result = validateCommitMessage(long, "disabled", 72);
    expect(result.warnings).toEqual([]);
  });

  test("rejects a multi-line message", () => {
    expect(() =>
      validateCommitMessage("feat: add thing\nmore text", "strict")
    ).toThrow(InvalidCommitMessageError);
  });
});

describe("tryExtractValidCommit", () => {
  test("finds a conventional-commit-shaped substring", () => {
    const message =
      "Sure, here you go: feat(ui): add dark mode toggle. Let me know if that works!";
    expect(tryExtractValidCommit(message, "strict")).toBe(
      "feat(ui): add dark mode toggle. Let me know if that works!"
    );
  });

  test("returns undefined when nothing salvageable exists", () => {
    expect(
      tryExtractValidCommit("no colon here at all", "strict")
    ).toBeUndefined();
  });
});

describe("validateOrSalvage", () => {
  test("salvages a message wrapped in commentary", () => {
    const result = validateOrSalvage("Sure! feat: add the thing", "strict");
    expect(result.salvaged).toBe(true);
    expect(result.message).toBe("feat: add the thing");
  });

  test("throws when nothing can be salvaged", () => {
    expect(() => validateOrSalvage("not a commit message", "strict")).toThrow(
      InvalidCommitMessageError
    );
  });
});
