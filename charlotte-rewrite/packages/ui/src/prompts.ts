import { isCancel, select, text } from "@clack/prompts";

/** Mirrors `select_action`'s option set from `christina/src/ui/mod.rs`. */
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

/** `undefined` on Ctrl-C/Esc, matching Christina's own cancel-to-decline
 * behavior at the call site. */
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
 * Mirrors `edit_commit_message_inline` from `christina/src/ui/mod.rs`.
 *
 * Open decision from `09-cli-and-ui.md`: Christina bound Ctrl-Left/Right to
 * jump by word (`bind_ctrl_word_navigation`, via `rustyline`).
 * `@clack/prompts`'s `text` prompt exposes no equivalent binding hook — its
 * `aliases` map covers action keys (cancel/up/down), not word-wise cursor
 * movement — so this drops the binding and uses the prompt's own default
 * line editing rather than hand-rolling raw keypress handling for it.
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
