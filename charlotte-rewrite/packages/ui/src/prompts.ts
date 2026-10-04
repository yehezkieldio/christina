import { isCancel, select, text } from "@clack/prompts";

/** The actions offered for a proposed commit message. */
export const COMMIT_ACTIONS = [
  "accept",
  "edit",
  "regenerate",
  "decline",
] as const;
export type CommitAction = (typeof COMMIT_ACTIONS)[number];

const ACTION_LABELS: Record<CommitAction, string> = {
  accept: "Accept",
  decline: "Decline",
  edit: "Edit inline",
  regenerate: "Regenerate",
};

/** `undefined` on Ctrl-C/Esc, which callers treat as a decline. */
export const selectCommitAction = async (): Promise<
  CommitAction | undefined
> => {
  const choice = await select({
    message: "What would you like to do?",
    options: COMMIT_ACTIONS.map((action) => ({
      label: ACTION_LABELS[action],
      value: action,
    })),
  });
  return isCancel(choice) ? undefined : choice;
};

const sanitizeInlineMessage = (message: string): string =>
  message.split("\n").join(" ");

/**
 * Inline commit message editor. Ctrl-Left/Right do not jump by word:
 * `@clack/prompts`'s `text` prompt exposes no binding hook for it — its
 * `aliases` map covers action keys (cancel/up/down), not word-wise cursor
 * movement — and hand-rolling raw keypress handling was rejected in favor of
 * the prompt's own default line editing.
 */
export const editCommitMessageInline = async (
  initialContent: string
): Promise<string | undefined> => {
  const result = await text({
    initialValue: sanitizeInlineMessage(initialContent),
    message: "Edit commit message",
    validate(value) {
      return (value ?? "").trim().length === 0
        ? "Commit message cannot be empty."
        : undefined;
    },
  });
  return isCancel(result) ? undefined : result.trim();
};
