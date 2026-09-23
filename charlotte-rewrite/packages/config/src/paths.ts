import { homedir } from "node:os";
import { join } from "node:path";

/** OS-appropriate XDG-style config directory, matching Christina's layout:
 * `~/.config/charlotte` on Linux and macOS, `%APPDATA%\charlotte` on Windows. */
export function configDir(env: Readonly<Record<string, string | undefined>> = process.env): string {
  if (process.platform === "win32") {
    return join(env["APPDATA"] ?? join(homedir(), "AppData", "Roaming"), "charlotte");
  }
  return join(env["XDG_CONFIG_HOME"] ?? join(homedir(), ".config"), "charlotte");
}

export function configFilePath(env?: Readonly<Record<string, string | undefined>>): string {
  return join(configDir(env), "config.toml");
}

export function profilesFilePath(env?: Readonly<Record<string, string | undefined>>): string {
  return join(configDir(env), "profiles.toml");
}

/** OS-appropriate XDG-style data directory, for the session transcripts in
 * `08-session-storage-and-stats.md`: `~/.local/share/charlotte` on Linux. */
export function dataDir(env: Readonly<Record<string, string | undefined>> = process.env): string {
  if (process.platform === "win32") {
    return join(env["LOCALAPPDATA"] ?? join(homedir(), "AppData", "Local"), "charlotte");
  }
  if (process.platform === "darwin") {
    return join(homedir(), "Library", "Application Support", "charlotte");
  }
  return join(env["XDG_DATA_HOME"] ?? join(homedir(), ".local", "share"), "charlotte");
}
