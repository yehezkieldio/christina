import { isCancel, select, text } from "@clack/prompts";

/** Mirrors `select_action`'s option set from `christina/src/ui/mod.rs`. */
export const COMMIT_ACTIONS = ["accept", "edit", "regenerate", "decline"] as const;
export type CommitAction = (typeof COMMIT_ACTIONS)[number];

const ACTION_LABELS: Record<CommitAction, string> = {
  accept: "Accept",
  edit: "Edit inline",
  regenerate: "Regenerate",
  decline: "Decline",
};

/** `undefined` on Ctrl-C/Esc, matching Christina's own cancel-to-decline
 * behavior at the call site. */
export async function selectCommitAction(): Promise<CommitAction | undefined> {
  const choice = await select({
    message: "What would you like to do?",
    options: COMMIT_ACTIONS.map((action) => ({ value: action, label: ACTION_LABELS[action] })),
  });
  return isCancel(choice) ? undefined : choice;
}

function sanitizeInlineMessage(message: string): string {
  return message.split("\n").join(" ");
}

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
export async function editCommitMessageInline(initialContent: string): Promise<string | undefined> {
  const result = await text({
    message: "Edit commit message",
    initialValue: sanitizeInlineMessage(initialContent),
    validate(value) {
      return (value ?? "").trim().length === 0 ? "Commit message cannot be empty." : undefined;
    },
  });
  return isCancel(result) ? undefined : result.trim();
}
