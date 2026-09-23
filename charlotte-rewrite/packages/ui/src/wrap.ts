/** Greedy word wrap, ported from `wrap_text` in `christina/src/ui/mod.rs`
 * and `ui-extractable/src/primitive/mod.rs`. A blank input line stays a
 * blank output line rather than being dropped, so callers can render it as
 * an empty bordered row. */
export function wrapText(text: string, width: number): string[] {
  const lines: string[] = [];
  for (const rawLine of text.split("\n")) {
    if (rawLine.length === 0) {
      lines.push("");
      continue;
    }

    let current = "";
    for (const word of rawLine.split(/\s+/).filter(Boolean)) {
      if (current.length === 0) {
        current = word;
        continue;
      }
      if (current.length + 1 + word.length > width) {
        lines.push(current);
        current = word;
      } else {
        current = `${current} ${word}`;
      }
    }
    if (current.length > 0) {
      lines.push(current);
    }
  }
  return lines;
}
