import { z } from "zod";

/** The six pipeline stages named in `01-architecture.md`'s data-flow
 * section, in run order. */
export const PIPELINE_STAGES = [
  "read",
  "configure",
  "contextualize",
  "analyze",
  "clean-and-match",
  "commit-and-record",
] as const;
export const pipelineStageSchema = z.enum(PIPELINE_STAGES);
export type PipelineStage = (typeof PIPELINE_STAGES)[number];

/** Structured warning data, per the open decision in
 * `07-orchestrator-pipeline.md`: the orchestrator returns this shape, not a
 * pre-formatted display string, so both the CLI and the transcript can
 * render it their own way. */
export const warningSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("truncation"), filePath: z.string(), droppedBytes: z.number().int().min(0) }),
  z.object({ kind: z.literal("salvage"), stage: pipelineStageSchema, reason: z.string() }),
  z.object({ kind: z.literal("fallback"), stage: pipelineStageSchema, reason: z.string() }),
  /** Two chunk summaries described opposing actions (e.g. "add" and
   * "remove") on the same change set. Ported from Christina's
   * `detect_contradictions`, which only logs; Charlotte surfaces it as
   * structured data instead so the CLI and transcript can both render it. */
  z.object({ kind: z.literal("contradiction"), action: z.string(), counteraction: z.string() }),
]);
export type Warning = z.infer<typeof warningSchema>;

const timestamped = { timestamp: z.iso.datetime() };

export const runOutcomeSchema = z.enum(["success", "declined", "aborted", "error"]);
export type RunOutcome = z.infer<typeof runOutcomeSchema>;

/**
 * The seven session transcript event types from
 * `08-session-storage-and-stats.md`. `type` is the discriminant: narrowing
 * a `SessionEvent` on it gives every other field its exact shape with no
 * cast, and `assertSessionEventExhaustive` below fails to compile if a
 * variant is ever added here without a matching `case` at every switch.
 */
export const sessionEventSchema = z.discriminatedUnion("type", [
  z.object({
    ...timestamped,
    type: z.literal("run_start"),
    sessionId: z.string(),
    repositoryPath: z.string(),
    configSummary: z.record(z.string(), z.unknown()),
  }),
  z.object({ ...timestamped, type: z.literal("stage_start"), stage: pipelineStageSchema }),
  z.object({
    ...timestamped,
    type: z.literal("stage_end"),
    stage: pipelineStageSchema,
    durationMs: z.number().min(0),
  }),
  z.object({
    ...timestamped,
    type: z.literal("request"),
    provider: z.string(),
    model: z.string(),
    promptTokens: z.number().int().min(0),
  }),
  z.object({
    ...timestamped,
    type: z.literal("response"),
    completionTokens: z.number().int().min(0),
    latencyMs: z.number().min(0),
  }),
  z.object({ ...timestamped, type: z.literal("retry"), attempt: z.number().int().min(1), reason: z.string() }),
  z.object({ ...timestamped, type: z.literal("warning"), warning: warningSchema }),
  z.object({
    ...timestamped,
    type: z.literal("run_end"),
    outcome: runOutcomeSchema,
    totalPromptTokens: z.number().int().min(0),
    totalCompletionTokens: z.number().int().min(0),
    finalMessageLength: z.number().int().min(0),
  }),
]);

export type SessionEvent = z.infer<typeof sessionEventSchema>;
export type SessionEventType = SessionEvent["type"];

/** Narrow the union to one variant by its discriminant literal, e.g.
 * `SessionEventOfType<"run_end">` is exactly the `run_end` event shape. */
export type SessionEventOfType<T extends SessionEventType> = Extract<SessionEvent, { type: T }>;

export function assertSessionEventExhaustive(event: never): never {
  throw new Error(`unreachable session event: ${JSON.stringify(event)}`);
}

export function isSessionEventOfType<T extends SessionEventType>(
  event: SessionEvent,
  type: T,
): event is SessionEventOfType<T> {
  return event.type === type;
}
