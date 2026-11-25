// The CSV export's columns (`export.rs`): every one in a file's own order, and the header each writes.

import type { CsvColumn, CsvHeaders } from "./api";

/** Every column, in the order a file has them unless the user chose another (`export::ALL`). */
export const CSV_COLUMNS: CsvColumn[] = [
  "seq",
  "status",
  "type",
  "name",
  "full_name",
  "set_name",
  "set_code",
  "card_number",
  "series",
  "year",
  "category",
  "confidence",
  "distance",
  "picked",
  "price",
  "currency",
  "grade",
  "condition",
  "slab_company",
  "slab_grade",
  "slab_certificate",
  "front",
  "back",
  "error",
  "notes",
  "location",
  "brand",
  "team",
  "serial",
  "features",
  "listing_title",
];

/** The column's header in the file: the user's name for it (`headers`), else its own, `#` for the card's number in
 * the session (`export::header_of`). */
export function csvHeader(column: CsvColumn, headers?: CsvHeaders): string {
  const renamed = headers?.[column]?.trim().replace(/\s+/g, " ");
  return renamed || (column === "seq" ? "#" : column);
}

/** `headers` with `column` named `name`; empty, or the column's own name, drops the rename. */
export function renamedTo(headers: CsvHeaders, column: CsvColumn, name: string): CsvHeaders {
  const out = { ...headers };
  const clean = name.trim().replace(/\s+/g, " ");
  if (!clean || clean === csvHeader(column)) delete out[column];
  else out[column] = clean;
  return out;
}

/** The headers a file would have twice. */
export function repeatedHeaders(columns: CsvColumn[], headers: CsvHeaders): string[] {
  const names = columns.map((c) => csvHeader(c, headers));
  return [...new Set(names.filter((n, i) => names.indexOf(n) !== i))];
}

/** The columns a file gets, as `export::columns` reads a choice: each known one once, none = all of them. */
export function csvColumnsOf(chosen: readonly string[] | null | undefined): CsvColumn[] {
  const out = [...new Set(chosen ?? [])].filter((c): c is CsvColumn => CSV_COLUMNS.includes(c as CsvColumn));
  return out.length ? out : [...CSV_COLUMNS];
}
