// A listing title template's pattern: words of the user's with `{part}`s in them, read as `ebay/title.rs` reads it.

import type { TitleField } from "./api";

/** Every part, in the order the editor offers them. */
export const TITLE_FIELDS: TitleField[] = [
  "year",
  "brand",
  "set",
  "name",
  "series",
  "features",
  "serial",
  "printRun",
  "number",
  "team",
  "grade",
  "game",
  "setCode",
];

/** A piece of a word: the user's text, or a part (`null`: a name no part has, `key` as written). */
export type Piece = { text: string } | { field: TitleField | null; key: string };

/** The part a `{key}` names, case aside. */
export function fieldOf(key: string): TitleField | null {
  const k = key.trim().toLowerCase();
  return TITLE_FIELDS.find((f) => f.toLowerCase() === k) ?? null;
}

/** A word split into its text and `{part}`s; `{{part}}` reads as `{part}`. */
export function piecesOf(word: string): Piece[] {
  const pieces: Piece[] = [];
  let rest = word;
  for (;;) {
    const open = rest.indexOf("{");
    if (open < 0) break;
    let braces = 0;
    while (rest[open + braces] === "{") braces++;
    const inner = rest.slice(open + braces);
    const close = inner.indexOf("}");
    if (close < 0) break;
    if (open > 0) pieces.push({ text: rest.slice(0, open) });
    const key = inner.slice(0, close).trim();
    pieces.push({ field: fieldOf(key), key });
    let after = inner.slice(close);
    let closing = 0;
    while (after[closing] === "}" && closing < braces) closing++;
    after = after.slice(closing);
    rest = after;
  }
  if (rest) pieces.push({ text: rest });
  return pieces;
}

/** The pattern's words, as written. */
export function wordsOf(pattern: string): string[] {
  return pattern.split(/\s+/).filter(Boolean);
}

/** The pattern a list of parts makes (a template of an older version): the number as `#{number}`. */
export function patternFromFields(fields: TitleField[]): string {
  return [...new Set(fields)]
    .filter((f) => TITLE_FIELDS.includes(f))
    .map((f) => (f === "number" ? "#{number}" : `{${f}}`))
    .join(" ");
}

/** The template's pattern, or the one its parts make. */
export function patternOf(template: { pattern?: string; fields: TitleField[] }): string {
  return template.pattern?.trim() ? template.pattern : patternFromFields(template.fields);
}

/** The parts a pattern names, each once, in order. */
export function fieldsIn(pattern: string): TitleField[] {
  const out: TitleField[] = [];
  for (const word of wordsOf(pattern))
    for (const p of piecesOf(word)) if ("field" in p && p.field && !out.includes(p.field)) out.push(p.field);
  return out;
}

/** The `{names}` in a pattern that no part has. */
export function unknownParts(pattern: string): string[] {
  const out: string[] = [];
  for (const word of wordsOf(pattern))
    for (const p of piecesOf(word)) if ("field" in p && !p.field && !out.includes(p.key)) out.push(p.key);
  return out;
}
