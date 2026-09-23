import { copyFileSync, mkdirSync } from "node:fs";
import path from "node:path";

import { suffix } from "bun:ffi";

/**
 * Compiles a standalone `charlotte` executable via `bun build --compile`.
 * The native crate has no cross-compilation setup, so this only targets the
 * host platform `cargo build --release` just produced — not the full
 * `bun-*` target matrix `bun build --compile` supports for pure-JS CLIs.
 */
const repoRoot = path.resolve(import.meta.dir, "../../..");
const nativeCrateDir = path.join(repoRoot, "native/charlotte-core");
const distDir = path.join(repoRoot, "dist");

const cargoBuild = Bun.spawnSync(
  [
    "cargo",
    "build",
    "--release",
    "--manifest-path",
    path.join(nativeCrateDir, "Cargo.toml"),
  ],
  { stderr: "inherit", stdout: "inherit" }
);
if (cargoBuild.exitCode !== 0) {
  process.exit(cargoBuild.exitCode ?? 1);
}

mkdirSync(distDir, { recursive: true });

const outfile = path.join(
  distDir,
  process.platform === "win32" ? "charlotte.exe" : "charlotte"
);
const bunBuild = Bun.spawnSync(
  [
    "bun",
    "build",
    path.join(import.meta.dir, "../src/cli.ts"),
    "--compile",
    "--outfile",
    outfile,
  ],
  { stderr: "inherit", stdout: "inherit" }
);
if (bunBuild.exitCode !== 0) {
  process.exit(bunBuild.exitCode ?? 1);
}

// Matches packages/native-core/src/library-path.ts's sidecar lookup: the
// compiled executable finds this file next to itself at runtime.
const libFileName =
  suffix === "dll" ? "charlotte_core.dll" : `libcharlotte_core.${suffix}`;
copyFileSync(
  path.join(nativeCrateDir, "target/release", libFileName),
  path.join(distDir, libFileName)
);

console.log(`Compiled: ${outfile}`);
console.log(`Native library: ${path.join(distDir, libFileName)}`);
