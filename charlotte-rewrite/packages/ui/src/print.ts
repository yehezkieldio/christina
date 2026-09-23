import { styles } from "./styles";
import { wrapText } from "./wrap";

/** The default terminal width `print_divider`/`print_commit_message` fall
 * back to when stdout is not a TTY (`process.stdout.columns` is
 * `undefined`), matching `console::Term::size()`'s own behavior of
 * returning a usable width even off a real terminal. */
const FALLBACK_COLUMNS = 80;
const COMMIT_MESSAGE_MAX_WIDTH = 96;
const DIVIDER_MAX_WIDTH = 80;

function terminalWidth(): number {
  return process.stdout.columns ?? FALLBACK_COLUMNS;
}

/** The nine print helpers from `christina/src/ui/mod.rs`: success, error,
 * warning, info, trace, section, divider, file list, and commit message. */
export function printSuccess(message: string): void {
  console.log(`${styles.muted("✓")} ${message}`);
}

export function printError(message: string): void {
  console.error(`${styles.error("×")} ${styles.error(message)}`);
}

export function printWarning(message: string): void {
  console.log(`${styles.warning("!")} ${message}`);
}

export function printInfo(message: string): void {
  console.log(`${styles.muted("•")} ${message}`);
}

export function printTrace(message: string): void {
  console.error(`${styles.muted("·")} ${styles.muted(message)}`);
}

export function printSection(title: string): void {
  console.log("");
  console.log(`${styles.muted("—")} ${styles.header(title)}`);
  console.log("");
}

export function printDivider(): void {
  const width = Math.min(terminalWidth(), DIVIDER_MAX_WIDTH);
  console.log(styles.muted("─".repeat(width)));
}

export function printFileList(files: readonly string[], maxItems: number): void {
  if (files.length === 0) {
    console.log(styles.muted("no files changed"));
    return;
  }

  const visible = Math.min(files.length, maxItems);
  for (const file of files.slice(0, visible)) {
    console.log(`${styles.muted("  ·")} ${styles.muted(file)}`);
  }

  const remaining = files.length - visible;
  if (remaining > 0) {
    console.log(styles.muted(`… ${remaining} more`));
  }
  console.log("");
}

export function printCommitMessage(message: string): void {
  const width = Math.max(Math.min(terminalWidth(), COMMIT_MESSAGE_MAX_WIDTH) - 6, 40);
  for (const line of wrapText(message, width)) {
    console.log(line.length === 0 ? styles.muted("│") : `${styles.muted("│")} ${line}`);
  }
  console.log("");
}
