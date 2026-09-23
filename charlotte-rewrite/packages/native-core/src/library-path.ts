import { existsSync } from "node:fs";
import path from "node:path";

import { suffix } from "bun:ffi";

// Unix crate output has a "lib" prefix; Windows does not.
const fileName =
  suffix === "dll" ? "charlotte_core.dll" : `libcharlotte_core.${suffix}`;

const CRATE_DIR = new URL("../../../native/charlotte-core/", import.meta.url);
const sourceTreePath = new URL(`target/release/${fileName}`, CRATE_DIR)
  .pathname;

/** `bun build --compile`'s output has no `import.meta.url`-relative crate
 * directory to resolve against — `apps/cli/scripts/compile.ts` ships the
 * native library as a sidecar file next to the compiled executable instead,
 * so a `dlopen`-able real path still exists on disk either way. `dlopen`
 * itself is why this can't just embed the library as a bundled asset: it
 * needs a real filesystem path, not Bun's virtual compiled-asset FS. */
const sidecarPath = path.join(path.dirname(process.execPath), fileName);

export const nativeCoreLibraryPath = existsSync(sidecarPath)
  ? sidecarPath
  : sourceTreePath;
