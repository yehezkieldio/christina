import { randomUUID } from "node:crypto";

import { loadConfig } from "@charlotte/config";
import type { ResolvedConfig } from "@charlotte/config";
import {
  chunkDiff,
  NativeCoreError,
  readStagedDiff,
} from "@charlotte/native-core";
import { NativeCommitHistoryProvider } from "@charlotte/native-core/native-commit-history-provider";
import { generateCommitMessage } from "@charlotte/orchestrator";
import type { GenerationResult } from "@charlotte/orchestrator";
import { optional, RequestLimiter, resolveModel } from "@charlotte/providers";
import type { PipelineStage, RunOutcome, Warning } from "@charlotte/schemas";
import { SessionWriter } from "@charlotte/session";
import {
  bindSpinnerToProgress,
  createSpinner,
  editCommitMessageInline,
  printCommitMessage,
  printDivider,
  printFileList,
  printInfo,
  printSection,
  printSuccess,
  printTrace,
  printWarning,
  selectCommitAction,
} from "@charlotte/ui";

export interface GenerateOptions {
  readonly yes: boolean;
  readonly context?: string;
  readonly dryRun: boolean;
  readonly trace: boolean;
}

const formatWarning = (warning: Warning): string => {
  switch (warning.kind) {
    case "truncation": {
      return `truncated ${warning.filePath} (dropped ${warning.droppedBytes} bytes)`;
    }
    case "salvage": {
      return `salvaged output during ${warning.stage}: ${warning.reason}`;
    }
    case "fallback": {
      return `fell back during ${warning.stage}: ${warning.reason}`;
    }
    case "contradiction": {
      return `contradictory changes detected: "${warning.action}" vs "${warning.counteraction}"`;
    }
    default: {
      return warning satisfies never;
    }
  }
};

const writeStage = async <T>(
  writer: SessionWriter,
  trace: boolean,
  stage: PipelineStage,
  run: () => Promise<T>
): Promise<T> => {
  await writer.write({
    stage,
    timestamp: new Date().toISOString(),
    type: "stage_start",
  });
  if (trace) {
    printTrace(`stage: ${stage}`);
  }
  const startedAt = performance.now();
  const result = await run();
  await writer.write({
    durationMs: performance.now() - startedAt,
    stage,
    timestamp: new Date().toISOString(),
    type: "stage_end",
  });
  return result;
};

interface StagedChanges {
  readonly diff: string;
  readonly files: string[];
}

const validateRepository = (repoPath: string): StagedChanges => {
  let staged: ReturnType<typeof readStagedDiff>;
  try {
    staged = readStagedDiff(repoPath);
  } catch (error) {
    if (
      error instanceof NativeCoreError &&
      error.message.includes("failed to open git repository")
    ) {
      throw new Error(
        "No git repository found in the current directory. Run this from the repository root.",
        { cause: error }
      );
    }
    throw error;
  }
  if (staged.files.length === 0) {
    throw new Error(
      "No staged changes to commit. Stage your changes and try again."
    );
  }
  return { diff: staged.diff, files: [...staged.files] };
};

const displayChanges = (files: readonly string[]): void => {
  printSection("Staged changes");
  printInfo(`${files.length} ${files.length === 1 ? "file" : "files"} staged`);
  printFileList(files, 10);
};

/** `run_start`'s `configSummary`, per `08-session-storage-and-stats.md`:
 * no event may carry an API key or a resolved secret value. `apiKey` is a
 * `SecretString`, which redacts itself through `toJSON` — passing it
 * through unchanged, rather than deleting the field, means the transcript
 * still records which of `apiKey`'s two shapes applied (unset vs. set)
 * without the writer needing special-case knowledge of this one field. */
const summarizeConfig = (config: ResolvedConfig): ResolvedConfig => ({
  ...config,
});

const HISTORY_PREVIEW_MAX = 50;

/** Best-effort: commit-history context enriches the prompt but is not load
 * bearing, so a git-history read failure (e.g. an unborn branch some other
 * process already handled at the native layer) degrades to no context
 * instead of failing the whole run. */
const buildHistoryContext = (
  repoPath: string,
  depth: number
): string | undefined => {
  if (depth <= 0) {
    return;
  }
  try {
    const commits = new NativeCommitHistoryProvider().getCommitHistory(
      repoPath,
      Math.min(depth, HISTORY_PREVIEW_MAX)
    );
    return commits.length === 0
      ? undefined
      : commits.map((commit) => `${commit.sha} ${commit.subject}`).join("\n");
  } catch {
    // Best-effort: an unreadable history (e.g. an unborn branch) degrades
    // to no context instead of failing the whole run.
    return undefined;
  }
};

interface GenerationContext {
  readonly diff: string;
  readonly repoPath: string;
  readonly userContext?: string;
  readonly writer: SessionWriter;
  readonly trace: boolean;
  readonly config: ResolvedConfig;
  /** Mutated in place by every `generateOnce` call, including regenerations
   * triggered from `confirmLoop` — `run_end`'s totals need every call's
   * usage summed, not just the last one. */
  readonly tokenUsage: {
    promptTokens: number;
    completionTokens: number;
    cacheReadTokens: number;
    cacheWriteTokens: number;
  };
}

/** Writes the `request`/`response` pair for one model call, per
 * `08-session-storage-and-stats.md`. `usage.startedAt`/`latencyMs` come from
 * `@charlotte/providers`, the only layer that actually times the call, since
 * these pairs are written once the whole pipeline stage completes rather
 * than live as each call happens. The request event's timestamp is that
 * recorded start time, not the moment this function runs; the response
 * event's timestamp is derived by adding `latencyMs` to it. */
const writeRequestUsage = async (
  writer: SessionWriter,
  usage: GenerationResult["requests"][number]
): Promise<void> => {
  await writer.write({
    model: usage.model,
    promptTokens: usage.promptTokens,
    provider: usage.provider,
    requestId: usage.requestId,
    timestamp: usage.startedAt,
    type: "request",
  });
  await writer.write({
    cacheReadTokens: usage.cacheReadTokens,
    cacheWriteTokens: usage.cacheWriteTokens,
    completionTokens: usage.completionTokens,
    latencyMs: usage.latencyMs,
    requestId: usage.requestId,
    timestamp: new Date(
      new Date(usage.startedAt).getTime() + usage.latencyMs
    ).toISOString(),
    type: "response",
  });
};

const generateOnce = async (
  ctx: GenerationContext
): Promise<GenerationResult> => {
  const { config } = ctx;
  const spinner = createSpinner();
  spinner.start("preparing diff for the model");
  const onProgress = bindSpinnerToProgress(spinner);

  const { chunks, historyContext } = await writeStage(
    ctx.writer,
    ctx.trace,
    "contextualize",
    () =>
      Promise.resolve({
        chunks: chunkDiff(
          ctx.diff,
          config.maxTokens,
          config.lockfileTokenLimit
        ),
        historyContext: buildHistoryContext(
          ctx.repoPath,
          config.commitHistoryDepth
        ),
      })
  );
  if (ctx.trace) {
    printTrace(`diff chunks: ${chunks.length}`);
  }

  const model = resolveModel(config);
  const limiter = new RequestLimiter({
    maxConcurrent: config.maxConcurrentRequests,
    requestsPerSecond: config.requestsPerSecond,
  });

  onProgress({ message: "generating commit message", stage: "analyze" });
  const result = await writeStage(ctx.writer, ctx.trace, "analyze", () =>
    generateCommitMessage(chunks, {
      concurrencyLimit: config.maxConcurrentRequests,
      context: {
        ...optional("userContext", ctx.userContext),
        ...optional("historyContext", historyContext),
      },
      limiter,
      maxLength: config.commitMessageMaxLength,
      maxPartialFailureRate: config.partialFailureRate,
      model,
      validationMode: config.commitValidationMode,
    })
  );

  spinner.stop("done");

  ctx.tokenUsage.promptTokens += result.promptTokens;
  ctx.tokenUsage.completionTokens += result.completionTokens;
  for (const usage of result.requests) {
    ctx.tokenUsage.cacheReadTokens += usage.cacheReadTokens;
    ctx.tokenUsage.cacheWriteTokens += usage.cacheWriteTokens;
  }

  // `result.requests` is already fully resolved by this point (the model
  // calls themselves ran concurrently inside the pipeline stage above), so
  // there is no latency win from firing these writes concurrently too —
  // sequential keeps `SessionWriter.write`'s per-call schema validation
  // failures attributable to one line instead of a `Promise.all` batch.
  // oxlint-disable no-await-in-loop
  for (const usage of result.requests) {
    await writeRequestUsage(ctx.writer, usage);
  }
  // oxlint-enable no-await-in-loop

  // Warnings must print and log in the order the pipeline produced them; the
  // list is a handful of entries at most, so sequential writes cost nothing
  // measurable.
  // oxlint-disable no-await-in-loop
  for (const warning of result.warnings) {
    printWarning(formatWarning(warning));
    await ctx.writer.write({
      timestamp: new Date().toISOString(),
      type: "warning",
      warning,
    });
  }
  // oxlint-enable no-await-in-loop

  return result;
};

type MessageState = "proposed" | "edited" | "regenerated";

/** The accept/edit/regenerate/decline loop from `christina/src/ui/mod.rs`'s
 * `select_action`. Returns the final message, or `undefined` on decline. */
const confirmLoop = async (
  initialMessage: string,
  ctx: GenerationContext,
  yes: boolean
): Promise<string | undefined> => {
  let message = initialMessage;
  let state: MessageState = "proposed";
  let showMessage = true;

  // Every await here waits on the next interactive choice or its result —
  // an interactive REPL loop is sequential by nature, there is nothing to
  // batch into a `Promise.all`.
  // oxlint-disable no-await-in-loop
  for (;;) {
    if (showMessage) {
      printSection(`Message · ${state}`);
      printCommitMessage(message);
    }
    showMessage = false;

    if (yes) {
      return message;
    }

    const action = await selectCommitAction();
    switch (action) {
      case "accept": {
        return message;
      }
      case "edit": {
        printInfo("Edit message (enter to save, esc to cancel).");
        const edited = await editCommitMessageInline(message);
        if (edited === undefined) {
          printInfo("Edit cancelled.");
        } else if (edited.trim() === message.trim()) {
          printInfo("No changes applied.");
        } else {
          message = edited;
          state = "edited";
          showMessage = true;
          printSuccess("Message updated.");
        }
        break;
      }
      case "regenerate": {
        ({ message } = await generateOnce(ctx));
        state = "regenerated";
        showMessage = true;
        break;
      }
      case "decline":
      case undefined: {
        return;
      }
      default: {
        return action satisfies never;
      }
    }
  }
  // oxlint-enable no-await-in-loop
};

const executeCommit = async (
  repoPath: string,
  message: string
): Promise<void> => {
  const proc = Bun.spawn(["git", "commit", "-m", message], {
    cwd: repoPath,
    stderr: "pipe",
    stdout: "pipe",
  });
  const [exitCode, stderr] = await Promise.all([
    proc.exited,
    new Response(proc.stderr).text(),
  ]);
  if (exitCode !== 0) {
    if (/gpg/iu.test(stderr)) {
      printWarning(
        "GPG signing failed. Configure your GPG key/agent or disable signing with: git config commit.gpgsign false"
      );
    }
    throw new Error(
      stderr.trim().length > 0
        ? stderr.trim()
        : `git commit exited with code ${exitCode}`
    );
  }

  const oid = await new Response(
    Bun.spawn(["git", "rev-parse", "HEAD"], { cwd: repoPath, stdout: "pipe" })
      .stdout
  ).text();
  printSuccess(`Created commit ${oid.trim().slice(0, 7)}`);
};

export const runGenerate = async (options: GenerateOptions): Promise<void> => {
  printDivider();

  const sessionId = randomUUID();
  const repoPath = process.cwd();
  const writer = new SessionWriter(sessionId);
  const startedAt = new Date().toISOString();

  let outcome: RunOutcome = "error";
  let finalMessageLength = 0;
  const tokenUsage = {
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    completionTokens: 0,
    promptTokens: 0,
  };

  try {
    const { diff, files } = await writeStage(
      writer,
      options.trace,
      "read",
      () => Promise.resolve(validateRepository(repoPath))
    );
    const config = await writeStage(writer, options.trace, "configure", () =>
      loadConfig()
    );

    await writer.write({
      configSummary: summarizeConfig(config),
      repositoryPath: repoPath,
      sessionId,
      timestamp: startedAt,
      type: "run_start",
    });

    displayChanges(files);

    const ctx: GenerationContext = {
      config,
      diff,
      repoPath,
      tokenUsage,
      trace: options.trace,
      writer,
      ...optional("userContext", options.context),
    };
    const result = await generateOnce(ctx);

    if (options.dryRun) {
      printSection("Dry run");
      printInfo("Commit not created.");
      outcome = "declined";
      finalMessageLength = result.message.length;
      return;
    }

    const message = await writeStage(
      writer,
      options.trace,
      "clean-and-match",
      () => confirmLoop(result.message, ctx, options.yes)
    );
    if (message === undefined) {
      printInfo("Commit cancelled.");
      outcome = "declined";
      return;
    }

    await writeStage(writer, options.trace, "commit-and-record", () =>
      executeCommit(repoPath, message)
    );
    outcome = "success";
    finalMessageLength = message.length;
  } catch (error) {
    outcome =
      error instanceof Error && error.name === "AbortError"
        ? "aborted"
        : "error";
    throw error;
  } finally {
    await writer.write({
      finalMessageLength,
      outcome,
      timestamp: new Date().toISOString(),
      totalCacheReadTokens: tokenUsage.cacheReadTokens,
      totalCacheWriteTokens: tokenUsage.cacheWriteTokens,
      totalCompletionTokens: tokenUsage.completionTokens,
      totalPromptTokens: tokenUsage.promptTokens,
      type: "run_end",
    });
  }
};
