import { styles } from "./styles";

const padToWidth = (text: string, width: number): string =>
  text.length >= width ? text : text + " ".repeat(width - text.length);

const formatRow = (
  cells: readonly string[],
  widths: readonly number[]
): string =>
  widths
    .map((width, index) => padToWidth(cells[index] ?? "", width))
    .join("  ");

/** Table rendering (see `09-cli-and-ui.md`). Used by `charlotte stats`'s
 * grouped totals. */
export const printTable = (
  headers: readonly string[],
  rows: readonly (readonly string[])[]
): void => {
  if (headers.length === 0 && rows.length === 0) {
    return;
  }

  const columns = Math.max(headers.length, ...rows.map((row) => row.length), 0);
  const widths = Array.from({ length: columns }, (_, index) => {
    const headerWidth = headers[index]?.length ?? 0;
    const cellWidth = Math.max(
      0,
      ...rows.map((row) => row[index]?.length ?? 0)
    );
    return Math.max(headerWidth, cellWidth);
  });

  console.log(styles.header(formatRow(headers, widths)));
  console.log(
    styles.muted(widths.map((width) => "─".repeat(width)).join("  "))
  );
  for (const row of rows) {
    console.log(formatRow(row, widths));
  }
  console.log("");
};
