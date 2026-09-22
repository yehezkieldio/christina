import { suffix } from "bun:ffi";

const CRATE_DIR = new URL("../../../native/charlotte-core/", import.meta.url);

// Unix crate output has a "lib" prefix; Windows does not.
const fileName = suffix === "dll" ? "charlotte_core.dll" : `libcharlotte_core.${suffix}`;

export const nativeCoreLibraryPath = new URL(`target/release/${fileName}`, CRATE_DIR).pathname;
