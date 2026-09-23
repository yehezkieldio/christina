import { defineConfig } from "oxlint";
import antiSlop from "ultracite/oxlint/anti-slop";
import core from "ultracite/oxlint/core";

// This project uses `bun:test` exclusively, never vitest — the `vitest`
// preset is deliberately not extended (its `prefer-importing-vitest-globals`
// rule misfires against `bun:test` imports of the same names).
export default defineConfig({
  extends: [core, antiSlop],
  // `.claude/skills/**` and `.agents/skills/**` hold vendored, third-party
  // skill assets (installer scripts, copied rule source) that this project
  // does not own and does not ship — they are not lint targets.
  ignorePatterns: [
    ...core.ignorePatterns,
    "**/.claude/skills/**",
    "**/.agents/skills/**",
  ],
});
