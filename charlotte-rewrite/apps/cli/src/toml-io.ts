import { mkdir } from "node:fs/promises";
import nodePath from "node:path";

import { parse as parseToml, stringify as stringifyToml } from "smol-toml";

/** Bun's `TOML.parse` (used by `@charlotte/config`'s read-only loader) has
 * no matching stringify, so the CLI's write path — the one place Charlotte
 * persists config or profile edits back to disk — uses `smol-toml` instead. */
export const readToml = async (
  path: string
): Promise<Record<string, unknown>> => {
  const file = Bun.file(path);
  if (!(await file.exists())) {
    return {};
  }
  return parseToml(await file.text()) as Record<string, unknown>;
};

export const writeToml = async (
  path: string,
  value: Record<string, unknown>
): Promise<void> => {
  await mkdir(nodePath.dirname(path), { recursive: true });
  await Bun.write(path, stringifyToml(value));
};
