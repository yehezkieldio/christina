import { styleText } from "node:util";

/**
 * The five named styles from `christina/src/ui/mod.rs` and
 * `ui-extractable/src/primitive/mod.rs`. `node:util`'s `styleText` degrades
 * to plain text when `NO_COLOR` is set or stdout is not a TTY, matching
 * `console::Style`'s own auto-detection, so callers never need to check
 * `process.stdout.isTTY` themselves.
 */
export const styles = {
  header: (text: string): string => styleText("bold", text),
  error: (text: string): string => styleText(["red", "bold"], text),
  warning: (text: string): string => styleText(["yellow", "bold"], text),
  accent: (text: string): string => styleText(["cyan", "bold"], text),
  muted: (text: string): string => styleText("dim", text),
} as const;
