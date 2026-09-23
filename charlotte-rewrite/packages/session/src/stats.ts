import type { SessionEvent } from "@charlotte/schemas";

export interface RunSummary {
  readonly sessionId: string;
  /** UTC calendar day, `YYYY-MM-DD`, taken from `run_start.timestamp`. */
  readonly day: string;
  readonly provider: string;
  readonly model: string;
  readonly inputTokens: number;
  readonly outputTokens: number;
}

const findEvent = <T extends SessionEvent["type"]>(
  events: readonly SessionEvent[],
  type: T
): Extract<SessionEvent, { type: T }> | undefined => {
  return events.find(
    (event): event is Extract<SessionEvent, { type: T }> => event.type === type
  );
};

/**
 * Reduces one run's transcript to the totals `charlotte stats` groups by.
 * Requires both `run_start` (day, session id) and `run_end` (token totals):
 * `run_end` already sums every `request`/`response` pair the orchestrator
 * saw, so re-summing from individual events here would just repeat that
 * work. A run with no `run_end` was interrupted mid-flight and contributes
 * nothing, the same way a crash leaves a partial file.
 */
export const summarizeRun = (
  events: readonly SessionEvent[]
): RunSummary | undefined => {
  const runStart = findEvent(events, "run_start");
  const runEnd = findEvent(events, "run_end");
  if (!(runStart && runEnd)) {
    return;
  }

  const firstRequest = findEvent(events, "request");
  return {
    day: runStart.timestamp.slice(0, 10),
    inputTokens: runEnd.totalPromptTokens,
    model: firstRequest?.model ?? "unknown",
    outputTokens: runEnd.totalCompletionTokens,
    provider: firstRequest?.provider ?? "unknown",
    sessionId: runStart.sessionId,
  };
};

export interface StatsGroup {
  readonly key: string;
  readonly runCount: number;
  readonly inputTokens: number;
  readonly outputTokens: number;
}

const groupBy = (
  summaries: readonly RunSummary[],
  keyOf: (summary: RunSummary) => string
): StatsGroup[] => {
  const totals = new Map<
    string,
    { runCount: number; inputTokens: number; outputTokens: number }
  >();
  for (const summary of summaries) {
    const key = keyOf(summary);
    const group = totals.get(key) ?? {
      inputTokens: 0,
      outputTokens: 0,
      runCount: 0,
    };
    group.runCount += 1;
    group.inputTokens += summary.inputTokens;
    group.outputTokens += summary.outputTokens;
    totals.set(key, group);
  }
  return [...totals.entries()]
    .map(([key, group]) => ({ key, ...group }))
    .toSorted((a, b) => a.key.localeCompare(b.key));
};

export interface Stats {
  readonly byDay: StatsGroup[];
  readonly byProvider: StatsGroup[];
  readonly byModel: StatsGroup[];
}

/**
 * `charlotte stats`'s grouping logic, per `08-session-storage-and-stats.md`:
 * total input tokens, total output tokens, and a run count, grouped by day,
 * by provider, and by model. Cost estimation is deliberately absent — the
 * spec defers it to a fast-follow because it needs per-provider pricing
 * data that changes over time.
 */
export const computeStats = (summaries: readonly RunSummary[]): Stats => ({
  byDay: groupBy(summaries, (summary) => summary.day),
  byModel: groupBy(summaries, (summary) => summary.model),
  byProvider: groupBy(summaries, (summary) => summary.provider),
});
