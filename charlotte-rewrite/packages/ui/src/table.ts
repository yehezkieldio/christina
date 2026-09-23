import { styles } from "./styles";

/** Table rendering, folded in from `ui-extractable/src/primitive/mod.rs`'s
 * `print_table` per `09-cli-and-ui.md` — Charlotte does not keep a second
 * compiled artifact for this, so it lives alongside the rest of the
 * styling logic instead of a standalone crate/package. Used by
 * `charlotte stats`'s grouped totals. */
export function printTable(headers: readonly string[], rows: readonly (readonly string[])[]): void {
  if (headers.length === 0 && rows.length === 0) {
    return;
  }

  const columns = Math.max(headers.length, ...rows.map((row) => row.length), 0);
  const widths = Array.from({ length: columns }, (_, index) => {
    const headerWidth = headers[index]?.length ?? 0;
    const cellWidth = Math.max(0, ...rows.map((row) => row[index]?.length ?? 0));
    return Math.max(headerWidth, cellWidth);
  });

  console.log(styles.header(formatRow(headers, widths)));
  console.log(styles.muted(widths.map((width) => "─".repeat(width)).join("  ")));
  for (const row of rows) {
    console.log(formatRow(row, widths));
  }
  console.log("");
}

function formatRow(cells: readonly string[], widths: readonly number[]): string {
  return widths.map((width, index) => padToWidth(cells[index] ?? "", width)).join("  ");
}

function padToWidth(text: string, width: number): string {
  return text.length >= width ? text : text + " ".repeat(width - text.length);
}
