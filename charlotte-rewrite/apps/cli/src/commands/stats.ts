import {
  computeStats,
  listSessionFiles,
  pruneSessions,
  readSessionFile,
  summarizeRun,
} from "@charlotte/session";
import type { RunSummary } from "@charlotte/session";
import { printInfo, printSection, printTable } from "@charlotte/ui";
import { command, constant, message, object } from "@optique/core";
import type { InferValue } from "@optique/core";

/** Reads every session file concurrently rather than one at a time: these
 * are independent filesystem reads with no shared state, so there is no
 * reason to pay N sequential round trips when they can overlap. Unlike a
 * network call, a local file read needs no concurrency cap. */
const collectRunSummaries = async (): Promise<RunSummary[]> => {
  const files = await listSessionFiles();
  const perFile = await Promise.all(
    files.map(async (file) => {
      const { events } = await readSessionFile(file);
      return summarizeRun(events);
    })
  );
  return perFile.filter(
    (summary): summary is RunSummary => summary !== undefined
  );
};

const printGroup = (
  title: string,
  headers: readonly string[],
  rows: readonly (readonly [string, number, number])[]
): void => {
  printSection(title);
  printTable(
    headers,
    rows.map(([key, inputTokens, outputTokens]) => [
      key,
      String(inputTokens),
      String(outputTokens),
    ])
  );
};

const handleStats = async (): Promise<void> => {
  const summaries = await collectRunSummaries();
  if (summaries.length === 0) {
    printInfo("No session data recorded yet.");
    return;
  }

  const stats = computeStats(summaries);
  const toRows = (groups: typeof stats.byDay) =>
    groups.map(
      (group) =>
        [
          `${group.key} (${group.runCount} run${group.runCount === 1 ? "" : "s"})`,
          group.inputTokens,
          group.outputTokens,
        ] as const
    );

  printGroup(
    "By day",
    ["Day", "Input tokens", "Output tokens"],
    toRows(stats.byDay)
  );
  printGroup(
    "By provider",
    ["Provider", "Input tokens", "Output tokens"],
    toRows(stats.byProvider)
  );
  printGroup(
    "By model",
    ["Model", "Input tokens", "Output tokens"],
    toRows(stats.byModel)
  );
};

const handleSessionsClean = async (): Promise<void> => {
  const removed = await pruneSessions();
  if (removed.length === 0) {
    printInfo("No session files needed pruning.");
    return;
  }
  printInfo(
    `Removed ${removed.length} old session file${removed.length === 1 ? "" : "s"}.`
  );
};

export const statsParser = command(
  "stats",
  object({ group: constant("stats") }),
  {
    description: message`Show token usage grouped by day, provider, and model`,
  }
);

export const sessionsParser = command(
  "sessions",
  object({
    action: command(
      "clean",
      object({ action: constant("clean") }),
      { description: message`Prune session files past the retention limit` }
    ),
    group: constant("sessions"),
  }),
  { description: message`Session transcript management` }
);

export type SessionsAction = InferValue<typeof sessionsParser>["action"];

export const runStatsAction = async (): Promise<void> => {
  await handleStats();
};

export const runSessionsAction = async (
  action: SessionsAction
): Promise<void> => {
  switch (action.action) {
    case "clean": {
      await handleSessionsClean();
      return;
    }
    default: {
      action.action satisfies never;
    }
  }
};
