import { defineConfig } from "oxlint";
import core from "ultracite/oxlint/core";
import vitest from "ultracite/oxlint/vitest";
import antiSlop from "ultracite/oxlint/anti-slop";

export default defineConfig({
  extends: [core, vitest, antiSlop],
  ignorePatterns: core.ignorePatterns,
});
