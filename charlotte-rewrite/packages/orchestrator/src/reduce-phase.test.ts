import { describe, expect, test } from "bun:test";
import { cleanResponse } from "./reduce-phase";

describe("cleanResponse", () => {
  test("passes through an already-clean message", () => {
    expect(cleanResponse("feat(auth): add OAuth flow")).toBe("feat(auth): add OAuth flow");
  });

  test("strips a markdown code fence", () => {
    expect(cleanResponse("```\nfeat: add thing\n```")).toBe("feat: add thing");
  });

  test("strips a known preamble", () => {
    expect(cleanResponse("Here is the commit message: feat: add thing")).toBe("feat: add thing");
  });

  test("keeps only the first line", () => {
    expect(cleanResponse("feat: add thing\n\nBody text here")).toBe("feat: add thing");
  });
});
