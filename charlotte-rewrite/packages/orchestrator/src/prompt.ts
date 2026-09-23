import type { ChunkSummary, ThemeItem } from "@charlotte/schemas";

/** Free-text context supplied by the operator (`--context`) and/or the
 * trimmed commit-history window, appended identically to every stage's
 * prompt — mirrors how Christina's `PromptBuilder` layers these on. */
export interface PromptContext {
  readonly userContext?: string;
  readonly historyContext?: string;
}

const SYSTEM_PROMPT =
  "You are an expert software engineer writing a single-line Conventional Commit message for a staged Git diff. Respond only with the requested structured fields, in the exact schema given.";

export const buildSystemPrompt = (): string => SYSTEM_PROMPT;

const appendContext = (prompt: string, context: PromptContext): string => {
  let result = prompt;
  if (context.userContext) {
    result += `\n\nAdditional context from the operator:\n${context.userContext}`;
  }
  if (context.historyContext) {
    result += `\n\nRecent commit history for style reference:\n${context.historyContext}`;
  }
  return result;
};

export const buildDirectPrompt = (
  diff: string,
  context: PromptContext = {}
): string => {
  const prompt = `Write a single-line Conventional Commit message for this diff:\n\n${diff}`;
  return appendContext(prompt, context);
};

export const buildChunkSummaryPrompt = (diffChunkContent: string): string =>
  `Summarize the following diff chunk in one sentence, focused on what changed and why:\n\n${diffChunkContent}`;

export const formatSummariesForPrompt = (
  summaries: readonly ChunkSummary[]
): string[] =>
  summaries.map(
    (summary) =>
      `[${summary.files.length} files: ${summary.files.join(", ")}] ${summary.summary}`
  );

export const buildIntentPrompt = (
  summaries: readonly ChunkSummary[]
): string => {
  const formatted = formatSummariesForPrompt(summaries);
  return `Group the following diff-chunk summaries into a small number of coherent themes. Each theme needs a title, a description, a file count, and an optional scope.\n\nSummaries:\n${formatted.map((line) => `- ${line}`).join("\n")}`;
};

const describeTheme = (
  theme: Pick<ThemeItem, "title" | "description" | "fileCount" | "scope">
): string => {
  const scoped = theme.scope ? ` (${theme.scope})` : "";
  return `- ${theme.title}${scoped}: ${theme.description} [${theme.fileCount} files]`;
};

export const buildReducePrompt = (
  themes: readonly ThemeItem[],
  context: PromptContext = {}
): string => {
  const prompt = `Synthesize the following themes into a single-line Conventional Commit message:\n\n${themes.map(describeTheme).join("\n")}`;
  return appendContext(prompt, context);
};
