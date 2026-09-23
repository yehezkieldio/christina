import path from "node:path";

import { dataDir } from "@charlotte/config";

/** The transcript directory from `08-session-storage-and-stats.md`:
 * `~/.local/share/charlotte/sessions` on Linux, with the OS-appropriate
 * `dataDir` root elsewhere. */
export const sessionsDir = (env?: Readonly<Record<string, string | undefined>>): string => path.join(dataDir(env), "sessions");

export const sessionFilePath = (sessionId: string,
env?: Readonly<Record<string, string | undefined>>): string => path.join(sessionsDir(env), `${sessionId}.jsonl`);
