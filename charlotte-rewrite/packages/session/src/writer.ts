import { appendFile, mkdir } from "node:fs/promises";
import { dirname } from "node:path";
import { type SessionEvent, sessionEventSchema } from "@charlotte/schemas";
import { sessionFilePath } from "./paths";

/**
 * Appends one JSON Lines event per call, per `08-session-storage-and-stats.md`:
 * a line is only ever appended, never rewritten, so a crash mid-run leaves
 * every already-written line valid. `node:fs`'s `appendFile` opens and
 * closes the file each call rather than holding a stream open, which is the
 * right tradeoff here — a generation run writes on the order of tens of
 * events, not a volume where the syscall overhead matters.
 */
export class SessionWriter {
  readonly sessionId: string;
  readonly #path: string;
  #directoryReady = false;

  constructor(sessionId: string, env?: Readonly<Record<string, string | undefined>>) {
    this.sessionId = sessionId;
    this.#path = sessionFilePath(sessionId, env);
  }

  get path(): string {
    return this.#path;
  }

  /** Validates before it ever reaches disk, so a malformed event throws in
   * the orchestrator instead of corrupting the transcript. */
  async write(event: SessionEvent): Promise<void> {
    sessionEventSchema.parse(event);
    if (!this.#directoryReady) {
      await mkdir(dirname(this.#path), { recursive: true });
      this.#directoryReady = true;
    }
    await appendFile(this.#path, `${JSON.stringify(event)}\n`, "utf8");
  }
}
