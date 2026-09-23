import { homedir } from "node:os";
import path from "node:path";

/** OS-appropriate XDG-style config directory, matching Christina's layout:
 * `~/.config/charlotte` on Linux and macOS, `%APPDATA%\charlotte` on Windows. */
export const configDir = (
  env: Readonly<Record<string, string | undefined>> = process.env
): string => {
  if (process.platform === "win32") {
    return path.join(
      env["APPDATA"] ?? path.join(homedir(), "AppData", "Roaming"),
      "charlotte"
    );
  }
  return path.join(
    env["XDG_CONFIG_HOME"] ?? path.join(homedir(), ".config"),
    "charlotte"
  );
};

export const configFilePath = (env?: Readonly<Record<string, string | undefined>>): string => path.join(configDir(env), "config.toml");

export const profilesFilePath = (env?: Readonly<Record<string, string | undefined>>): string => path.join(configDir(env), "profiles.toml");

/** OS-appropriate XDG-style data directory, for the session transcripts in
 * `08-session-storage-and-stats.md`: `~/.local/share/charlotte` on Linux. */
export const dataDir = (
  env: Readonly<Record<string, string | undefined>> = process.env
): string => {
  if (process.platform === "win32") {
    return path.join(
      env["LOCALAPPDATA"] ?? path.join(homedir(), "AppData", "Local"),
      "charlotte"
    );
  }
  if (process.platform === "darwin") {
    return path.join(homedir(), "Library", "Application Support", "charlotte");
  }
  return path.join(
    env["XDG_DATA_HOME"] ?? path.join(homedir(), ".local", "share"),
    "charlotte"
  );
};
