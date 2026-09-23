import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { type SessionEvent, sessionEventSchema } from "@charlotte/schemas";
import { sessionsDir } from "./paths";

export interface ReadSessionResult {
  readonly events: SessionEvent[];
  /** A line that failed to parse as JSON or failed schema validation. A
   * process crash mid-write can leave a partial last line — per
   * `08-session-storage-and-stats.md`, every earlier line stays valid — so
   * this is a count to report, not an error to throw. */
  readonly malformedLines: number;
}

function parseLine(line: string): SessionEvent | undefined {
  let raw: unknown;
  try {
    raw = JSON.parse(line);
  } catch {
    return undefined;
  }
  const result = sessionEventSchema.safeParse(raw);
  return result.success ? result.data : undefined;
}

export async function readSessionFile(path: string): Promise<ReadSessionResult> {
  const file = Bun.file(path);
  if (!(await file.exists())) {
    return { events: [], malformedLines: 0 };
  }

  const events: SessionEvent[] = [];
  let malformedLines = 0;
  for (const line of (await file.text()).split("\n")) {
    if (line.length === 0) {
      continue;
    }
    const event = parseLine(line);
    if (event) {
      events.push(event);
    } else {
      malformedLines += 1;
    }
  }
  return { events, malformedLines };
}

export interface ListSessionFilesOptions {
  readonly dir?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
}

export async function listSessionFiles(options: ListSessionFilesOptions = {}): Promise<string[]> {
  const target = options.dir ?? sessionsDir(options.env);
  try {
    const entries = await readdir(target);
    return entries
      .filter((name) => name.endsWith(".jsonl"))
      .map((name) => join(target, name))
      .sort();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return [];
    }
    throw error;
  }
}
