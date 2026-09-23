import { computeStats, listSessionFiles, pruneSessions, readSessionFile, type RunSummary, summarizeRun } from "@charlotte/session";
import { printInfo, printSection, printTable } from "@charlotte/ui";
import type { Command } from "commander";

async function collectRunSummaries(): Promise<RunSummary[]> {
  const files = await listSessionFiles();
  const summaries: RunSummary[] = [];
  for (const file of files) {
    const { events } = await readSessionFile(file);
    const summary = summarizeRun(events);
    if (summary) {
      summaries.push(summary);
    }
  }
  return summaries;
}

function printGroup(title: string, headers: readonly string[], rows: readonly (readonly [string, number, number])[]): void {
  printSection(title);
  printTable(
    headers,
    rows.map(([key, inputTokens, outputTokens]) => [key, String(inputTokens), String(outputTokens)]),
  );
}

async function handleStats(): Promise<void> {
  const summaries = await collectRunSummaries();
  if (summaries.length === 0) {
    printInfo("No session data recorded yet.");
    return;
  }

  const stats = computeStats(summaries);
  const toRows = (groups: typeof stats.byDay) =>
    groups.map((group) => [`${group.key} (${group.runCount} run${group.runCount === 1 ? "" : "s"})`, group.inputTokens, group.outputTokens] as const);

  printGroup("By day", ["Day", "Input tokens", "Output tokens"], toRows(stats.byDay));
  printGroup("By provider", ["Provider", "Input tokens", "Output tokens"], toRows(stats.byProvider));
  printGroup("By model", ["Model", "Input tokens", "Output tokens"], toRows(stats.byModel));
}

async function handleSessionsClean(): Promise<void> {
  const removed = await pruneSessions();
  if (removed.length === 0) {
    printInfo("No session files needed pruning.");
    return;
  }
  printInfo(`Removed ${removed.length} old session file${removed.length === 1 ? "" : "s"}.`);
}

export function registerStatsCommand(program: Command): void {
  program
    .command("stats")
    .description("Show token usage grouped by day, provider, and model")
    .action(async () => {
      await handleStats();
    });

  const sessions = program.command("sessions").description("Session transcript management");
  sessions
    .command("clean")
    .description("Prune session files past the retention limit")
    .action(async () => {
      await handleSessionsClean();
    });
}
