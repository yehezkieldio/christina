import { suffix } from "bun:ffi";

// Unix crate output has a "lib" prefix; Windows does not.
const fileName =
  suffix === "dll" ? "charlotte_core.dll" : `libcharlotte_core.${suffix}`;

const CRATE_DIR = new URL("../../../native/charlotte-core/", import.meta.url);
const sourceTreePath = new URL(`target/release/${fileName}`, CRATE_DIR)
  .pathname;

/**
 * `apps/cli/scripts/compile.ts` generates `./native-lib.generated.ts` right
 * before invoking `bun build --compile`: a `with { type: "file" }` import of
 * the current platform's compiled library, the one import shape Bun
 * materializes to a real on-disk path at runtime — which `dlopen` needs and
 * Bun's virtual compiled-asset filesystem cannot provide on its own. Running
 * from source (`bun run`) has no such generated file, so this falls back to
 * resolving the crate's own build output directly.
 */
const embeddedPath = await import("./native-lib.generated")
  .then((mod) => mod.default)
  .catch(() => {
    /* no embedded library in this build; fall back to the source tree */
  });

export const nativeCoreLibraryPath = embeddedPath ?? sourceTreePath;
