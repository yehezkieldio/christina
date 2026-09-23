import { stat, unlink } from "node:fs/promises";
import { listSessionFiles } from "./reader";
import { sessionsDir } from "./paths";

/**
 * Retention policy chosen for `08-session-storage-and-stats.md`'s open
 * decision: a maximum file count, not a maximum age. A count cap gives a
 * predictable disk-usage ceiling without parsing a filename or an event
 * timestamp to decide what "old" means, and it is the policy the spec
 * names first. `charlotte sessions clean` (phase 5) calls this directly;
 * nothing calls it automatically, so a run's own transcript is never at
 * risk of being pruned mid-write.
 */
const DEFAULT_MAX_FILES = 200;

export interface PruneOptions {
  readonly dir?: string;
  readonly maxFiles?: number;
  readonly env?: Readonly<Record<string, string | undefined>>;
}

export async function pruneSessions(options: PruneOptions = {}): Promise<string[]> {
  const dir = options.dir ?? sessionsDir(options.env);
  const maxFiles = options.maxFiles ?? DEFAULT_MAX_FILES;

  const files = await listSessionFiles({ dir });
  if (files.length <= maxFiles) {
    return [];
  }

  const withMtime = await Promise.all(
    files.map(async (path) => ({ path, mtimeMs: (await stat(path)).mtimeMs })),
  );
  withMtime.sort((a, b) => a.mtimeMs - b.mtimeMs);

  const toRemove = withMtime.slice(0, withMtime.length - maxFiles);
  await Promise.all(toRemove.map((entry) => unlink(entry.path)));
  return toRemove.map((entry) => entry.path);
}
