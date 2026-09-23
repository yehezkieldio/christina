import { join } from "node:path";
import { dataDir } from "@charlotte/config";

/** The transcript directory from `08-session-storage-and-stats.md`:
 * `~/.local/share/charlotte/sessions` on Linux, with the OS-appropriate
 * `dataDir` root elsewhere. */
export function sessionsDir(env?: Readonly<Record<string, string | undefined>>): string {
  return join(dataDir(env), "sessions");
}

export function sessionFilePath(sessionId: string, env?: Readonly<Record<string, string | undefined>>): string {
  return join(sessionsDir(env), `${sessionId}.jsonl`);
}
