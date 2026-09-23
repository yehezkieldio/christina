import { randomUUID } from "node:crypto";
import { loadConfig, type ResolvedConfig } from "@charlotte/config";
import { chunkDiff, NativeCoreError, readStagedDiff } from "@charlotte/native-core";
import { NativeCommitHistoryProvider } from "@charlotte/native-core/native-commit-history-provider";
import { generateCommitMessage, type GenerationResult } from "@charlotte/orchestrator";
import { resolveModel } from "@charlotte/providers";
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

function formatWarning(warning: Warning): string {
  switch (warning.kind) {
    case "truncation":
      return `truncated ${warning.filePath} (dropped ${warning.droppedBytes} bytes)`;
    case "salvage":
      return `salvaged output during ${warning.stage}: ${warning.reason}`;
    case "fallback":
      return `fell back during ${warning.stage}: ${warning.reason}`;
    case "contradiction":
      return `contradictory changes detected: "${warning.action}" vs "${warning.counteraction}"`;
    default:
      return warning satisfies never;
  }
}

async function writeStage<T>(writer: SessionWriter, trace: boolean, stage: PipelineStage, run: () => Promise<T>): Promise<T> {
  await writer.write({ type: "stage_start", timestamp: new Date().toISOString(), stage });
  if (trace) {
    printTrace(`stage: ${stage}`);
  }
  const startedAt = performance.now();
  const result = await run();
  await writer.write({ type: "stage_end", timestamp: new Date().toISOString(), stage, durationMs: performance.now() - startedAt });
  return result;
}

async function validateRepository(repoPath: string): Promise<{ diff: string; files: string[] }> {
  let staged: ReturnType<typeof readStagedDiff>;
  try {
    staged = readStagedDiff(repoPath);
  } catch (error) {
    if (error instanceof NativeCoreError && error.message.includes("failed to open git repository")) {
      throw new Error("No git repository found in the current directory. Run this from the repository root.");
    }
    throw error;
  }
  if (staged.files.length === 0) {
    throw new Error("No staged changes to commit. Stage your changes and try again.");
  }
  return { diff: staged.diff, files: [...staged.files] };
}

function displayChanges(files: readonly string[]): void {
  printSection("Staged changes");
  printInfo(`${files.length} ${files.length === 1 ? "file" : "files"} staged`);
  printFileList(files, 10);
}

/** `run_start`'s `configSummary`, per `08-session-storage-and-stats.md`:
 * no event may carry an API key or a resolved secret value. `apiKey` is a
 * `SecretString`, which redacts itself through `toJSON` — passing it
 * through unchanged, rather than deleting the field, means the transcript
 * still records which of `apiKey`'s two shapes applied (unset vs. set)
 * without the writer needing special-case knowledge of this one field. */
function summarizeConfig(config: ResolvedConfig): Record<string, unknown> {
  return { ...config };
}

const HISTORY_PREVIEW_MAX = 50;

/** Best-effort: commit-history context enriches the prompt but is not load
 * bearing, so a git-history read failure (e.g. an unborn branch some other
 * process already handled at the native layer) degrades to no context
 * instead of failing the whole run. */
async function buildHistoryContext(repoPath: string, depth: number): Promise<string | undefined> {
  if (depth <= 0) {
    return;
  }
  try {
    const commits = new NativeCommitHistoryProvider().getCommitHistory(repoPath, Math.min(depth, HISTORY_PREVIEW_MAX));
    return commits.length === 0 ? undefined : commits.map((commit) => `${commit.sha} ${commit.subject}`).join("\n");
  } catch {
    return;
  }
}

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
  readonly tokenUsage: { promptTokens: number; completionTokens: number };
}

async function generateOnce(ctx: GenerationContext): Promise<GenerationResult> {
  const config = ctx.config;
  const spinner = createSpinner();
  spinner.start("preparing diff for the model");
  const onProgress = bindSpinnerToProgress(spinner);

  const { chunks, historyContext } = await writeStage(ctx.writer, ctx.trace, "contextualize", async () => ({
    chunks: chunkDiff(ctx.diff, config.maxTokens, config.lockfileTokenLimit),
    historyContext: await buildHistoryContext(ctx.repoPath, config.commitHistoryDepth),
  }));
  if (ctx.trace) {
    printTrace(`diff chunks: ${chunks.length}`);
  }

  const model = resolveModel(config);

  onProgress({ stage: "analyze", message: "generating commit message" });
  const result = await writeStage(ctx.writer, ctx.trace, "analyze", () =>
    generateCommitMessage(chunks, {
      model,
      context: { userContext: ctx.userContext, historyContext },
      validationMode: config.commitValidationMode,
      maxLength: config.commitMessageMaxLength,
      concurrencyLimit: config.maxConcurrentRequests,
      maxPartialFailureRate: config.partialFailureRate,
    }),
  );

  spinner.stop("done");

  ctx.tokenUsage.promptTokens += result.promptTokens;
  ctx.tokenUsage.completionTokens += result.completionTokens;

  for (const warning of result.warnings) {
    printWarning(formatWarning(warning));
    await ctx.writer.write({ type: "warning", timestamp: new Date().toISOString(), warning });
  }

  return result;
}

type MessageState = "proposed" | "edited" | "regenerated";

/** The accept/edit/regenerate/decline loop from `christina/src/ui/mod.rs`'s
 * `select_action`. Returns the final message, or `undefined` on decline. */
async function confirmLoop(initialMessage: string, ctx: GenerationContext, yes: boolean): Promise<string | undefined> {
  let message = initialMessage;
  let state: MessageState = "proposed";
  let showMessage = true;

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
      case "accept":
        return message;
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
        const result = await generateOnce(ctx);
        message = result.message;
        state = "regenerated";
        showMessage = true;
        break;
      }
      case "decline":
      case undefined:
        return;
      default:
        return action satisfies never;
    }
  }
}

async function executeCommit(repoPath: string, message: string): Promise<void> {
  const proc = Bun.spawn(["git", "commit", "-m", message], { cwd: repoPath, stdout: "pipe", stderr: "pipe" });
  const [exitCode, stderr] = await Promise.all([proc.exited, new Response(proc.stderr).text()]);
  if (exitCode !== 0) {
    if (/gpg/i.test(stderr)) {
      printWarning("GPG signing failed. Configure your GPG key/agent or disable signing with: git config commit.gpgsign false");
    }
    throw new Error(stderr.trim().length > 0 ? stderr.trim() : `git commit exited with code ${exitCode}`);
  }

  const oid = await new Response(Bun.spawn(["git", "rev-parse", "HEAD"], { cwd: repoPath, stdout: "pipe" }).stdout).text();
  printSuccess(`Created commit ${oid.trim().slice(0, 7)}`);
}

export async function runGenerate(options: GenerateOptions): Promise<void> {
  printDivider();

  const sessionId = randomUUID();
  const repoPath = process.cwd();
  const writer = new SessionWriter(sessionId);
  const startedAt = new Date().toISOString();

  let outcome: RunOutcome = "error";
  let finalMessageLength = 0;
  const tokenUsage = { promptTokens: 0, completionTokens: 0 };

  try {
    const { diff, files } = await writeStage(writer, options.trace, "read", () => validateRepository(repoPath));
    const config = await writeStage(writer, options.trace, "configure", () => loadConfig());

    await writer.write({
      type: "run_start",
      timestamp: startedAt,
      sessionId,
      repositoryPath: repoPath,
      configSummary: summarizeConfig(config),
    });

    displayChanges(files);

    const ctx: GenerationContext = { diff, repoPath, userContext: options.context, writer, trace: options.trace, tokenUsage, config };
    const result = await generateOnce(ctx);

    if (options.dryRun) {
      printSection("Dry run");
      printInfo("Commit not created.");
      outcome = "declined";
      finalMessageLength = result.message.length;
      return;
    }

    const message = await writeStage(writer, options.trace, "clean-and-match", () => confirmLoop(result.message, ctx, options.yes));
    if (message === undefined) {
      printInfo("Commit cancelled.");
      outcome = "declined";
      return;
    }

    await writeStage(writer, options.trace, "commit-and-record", () => executeCommit(repoPath, message));
    outcome = "success";
    finalMessageLength = message.length;
  } catch (error) {
    outcome = error instanceof Error && error.name === "AbortError" ? "aborted" : "error";
    throw error;
  } finally {
    await writer.write({
      type: "run_end",
      timestamp: new Date().toISOString(),
      outcome,
      totalPromptTokens: tokenUsage.promptTokens,
      totalCompletionTokens: tokenUsage.completionTokens,
      finalMessageLength,
    });
  }
}
