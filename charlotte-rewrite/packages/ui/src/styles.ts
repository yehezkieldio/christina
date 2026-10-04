import { styleText } from "node:util";

/**
 * The five named styles. `node:util`'s `styleText` degrades to plain text
 * when `NO_COLOR` is set or stdout is not a TTY, so callers never need to check
 * `process.stdout.isTTY` themselves.
 */
export const styles = {
  accent: (text: string): string => styleText(["cyan", "bold"], text),
  error: (text: string): string => styleText(["red", "bold"], text),
  header: (text: string): string => styleText("bold", text),
  muted: (text: string): string => styleText("dim", text),
  warning: (text: string): string => styleText(["yellow", "bold"], text),
} as const;
