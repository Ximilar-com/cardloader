// Settings › Selling › Listing titles: the user's title templates, each a pattern of parts and the user's own words
// for the cards it is for, edited as chips or as text, with the titles they give shown as they are edited.

import clsx from "clsx";
import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Plus, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { t as translate, useT } from "../i18n";
import { api, type TitleField, type TitlePreview, type TitleTemplate } from "../lib/api";
import { fileUrl } from "../lib/format";
import { fieldsIn, patternOf, piecesOf, unknownParts, wordsOf } from "../lib/titlePattern";
import Select, { type SelectOption } from "./Select";
import { leftOutText, TITLE_FIELDS, TitleCount, titleFieldLabel } from "./TitleParts";

/** What a new template starts with: a sports title, name first, with what the built-in one leaves out. */
const STARTING_PATTERN = "{name} {year} {set} {series} {features} {printRun} #{number} {grade}";

/** A word of a pattern as a chip shows it: its parts by their names ("#Card number"). */
function wordLabel(word: string): string {
  return piecesOf(word)
    .map((p) => ("text" in p ? p.text : p.field ? titleFieldLabel(p.field) : `{${p.key}}`))
    .join("");
}

/** How a part is written into a pattern. */
function placeholder(field: TitleField): string {
  return field === "number" ? "#{number}" : `{${field}}`;
}

function kindOptions(): SelectOption<TitleTemplate["kind"]>[] {
  return [
    { value: "any", label: translate("titles.kind.any") },
    { value: "sport", label: translate("titles.kind.sport") },
    { value: "tcg", label: translate("titles.kind.tcg") },
    { value: "comics", label: translate("titles.kind.comics") },
  ];
}

function onlyOptions(): SelectOption<TitleTemplate["only"]>[] {
  return [
    { value: "any", label: translate("titles.only.any") },
    { value: "graded", label: translate("titles.only.graded"), hint: translate("titles.only.gradedHint") },
    { value: "autograph", label: translate("titles.only.autograph"), hint: translate("titles.only.autographHint") },
    { value: "parallel", label: translate("titles.only.parallel"), hint: translate("titles.only.parallelHint") },
    { value: "regular", label: translate("titles.only.regular"), hint: translate("titles.only.regularHint") },
  ];
}

/** `list` with the item at `from` moved by `by` places; unchanged at an end. */
function moved<T>(list: T[], from: number, by: number): T[] {
  const to = from + by;
  if (to < 0 || to >= list.length) return list;
  const out = [...list];
  out.splice(to, 0, ...out.splice(from, 1));
  return out;
}

export default function TitleTemplates({
  templates,
  onChange,
}: {
  templates: TitleTemplate[];
  onChange: (templates: TitleTemplate[]) => void;
}) {
  const t = useT();
  const [shown, setShown] = useState<TitlePreview[] | null>(null);

  // The titles come from the app itself, so what is shown is what a listing gets; a moment after the last change.
  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => {
      api
        .previewTitles({ templates })
        .then((titles) => live && setShown(titles))
        .catch(() => live && setShown([]));
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [templates]);

  const change = (at: number, patch: Partial<TitleTemplate>) =>
    onChange(templates.map((tpl, i) => (i === at ? { ...tpl, ...patch } : tpl)));
  const add = () =>
    onChange([
      ...templates,
      {
        id: crypto.randomUUID(),
        name: t("titles.newName", { n: templates.length + 1 }),
        kind: "any",
        only: "any",
        pattern: STARTING_PATTERN,
        fields: fieldsIn(STARTING_PATTERN),
      },
    ]);

  return (
    <div className="space-y-4">
      {templates.map((tpl, at) => (
        <Template
          key={tpl.id}
          template={tpl}
          first={at === 0}
          last={at === templates.length - 1}
          onChange={(patch) => change(at, patch)}
          onMove={(by) => onChange(moved(templates, at, by))}
          onRemove={() => onChange(templates.filter((_, i) => i !== at))}
        />
      ))}
      <button className="btn" onClick={add}>
        <Plus className="h-4 w-4" /> {t("titles.add")}
      </button>

      {shown && shown.length > 0 && (
        <div>
          <div className="label">{t("titles.preview.title")}</div>
          <p className="mb-2 text-xs text-slate-500">
            {shown[0].itemId == null ? t("titles.preview.examples") : t("titles.preview.yours")}
          </p>
          <ul className="divide-y divide-slate-100 rounded-md border border-slate-200 dark:divide-slate-800 dark:border-slate-800">
            {shown.map((row, i) => (
              <PreviewRow key={row.itemId ?? `example-${i}`} row={row} />
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function Template({
  template: tpl,
  first,
  last,
  onChange,
  onMove,
  onRemove,
}: {
  template: TitleTemplate;
  first: boolean;
  last: boolean;
  onChange: (patch: Partial<TitleTemplate>) => void;
  onMove: (by: number) => void;
  onRemove: () => void;
}) {
  const t = useT();
  return (
    <div className="space-y-3 rounded-md border border-slate-200 p-3 dark:border-slate-800">
      <div className="flex flex-wrap items-end gap-2">
        <div className="min-w-40 flex-1">
          <label className="label">{t("titles.name")}</label>
          <input
            className="input"
            value={tpl.name}
            onChange={(e) => onChange({ name: e.target.value })}
            aria-label={t("titles.name")}
          />
        </div>
        <div className="w-44">
          <div className="label">{t("titles.kind.label")}</div>
          <Select
            value={tpl.kind}
            options={kindOptions()}
            onChange={(kind) => onChange({ kind })}
            aria-label={t("titles.kind.label")}
          />
        </div>
        <div className="w-44">
          <div className="label">{t("titles.only.label")}</div>
          <Select
            value={tpl.only}
            options={onlyOptions()}
            onChange={(only) => onChange({ only })}
            aria-label={t("titles.only.label")}
          />
        </div>
        <div className="flex">
          <button
            className="btn btn-ghost px-1.5"
            disabled={first}
            onClick={() => onMove(-1)}
            title={t("titles.moveUp")}
            aria-label={t("titles.moveUp")}
          >
            <ArrowUp className="h-4 w-4" />
          </button>
          <button
            className="btn btn-ghost px-1.5"
            disabled={last}
            onClick={() => onMove(1)}
            title={t("titles.moveDown")}
            aria-label={t("titles.moveDown")}
          >
            <ArrowDown className="h-4 w-4" />
          </button>
          <button
            className="btn btn-ghost px-1.5"
            onClick={onRemove}
            title={t("titles.remove")}
            aria-label={t("titles.remove")}
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </div>

      <Pattern template={tpl} onChange={onChange} />
    </div>
  );
}

/** The template's pattern, as chips to move and remove and as text to write. */
function Pattern({
  template: tpl,
  onChange,
}: {
  template: TitleTemplate;
  onChange: (patch: Partial<TitleTemplate>) => void;
}) {
  const t = useT();
  const [text, setText] = useState("");
  const pattern = patternOf(tpl);
  const words = wordsOf(pattern);
  const used = fieldsIn(pattern);
  const unknown = unknownParts(pattern);
  const unused = TITLE_FIELDS.filter((f) => !used.includes(f));
  const set = (next: string) => onChange({ pattern: next, fields: fieldsIn(next) });
  const setWords = (next: string[]) => set(next.join(" "));
  const addText = () => {
    const added = wordsOf(text);
    if (added.length) setWords([...words, ...added]);
    setText("");
  };
  return (
    <div className="space-y-3">
      <div>
        <div className="label">{t("titles.parts")}</div>
        <div className="flex flex-wrap items-center gap-1.5">
          {words.map((word, at) => {
            const pieces = piecesOf(word);
            const own = pieces.every((p) => "text" in p);
            const odd = pieces.some((p) => "field" in p && !p.field);
            const label = own ? `“${word}”` : wordLabel(word);
            return (
              <span
                key={`${at}-${word}`}
                className={clsx(
                  "inline-flex items-center rounded-md border text-sm",
                  odd
                    ? "border-amber-400 bg-amber-50 dark:border-amber-700 dark:bg-amber-950"
                    : own
                      ? "border-dashed border-slate-300 dark:border-slate-700"
                      : "border-slate-300 bg-slate-50 dark:border-slate-700 dark:bg-slate-800",
                )}
              >
                <button
                  className="rounded-l-md px-0.5 py-1 text-slate-400 hover:bg-slate-200 hover:text-slate-700 disabled:opacity-30 dark:hover:bg-slate-700 dark:hover:text-slate-200"
                  disabled={at === 0}
                  onClick={() => setWords(moved(words, at, -1))}
                  aria-label={t("titles.moveEarlier", { field: label })}
                  title={t("titles.moveEarlier", { field: label })}
                >
                  <ChevronLeft className="h-3.5 w-3.5" />
                </button>
                <span className={clsx("px-1", own ? "text-slate-600 italic dark:text-slate-300" : "font-medium")}>
                  {label}
                </span>
                <button
                  className="px-0.5 py-1 text-slate-400 hover:bg-slate-200 hover:text-slate-700 disabled:opacity-30 dark:hover:bg-slate-700 dark:hover:text-slate-200"
                  disabled={at === words.length - 1}
                  onClick={() => setWords(moved(words, at, 1))}
                  aria-label={t("titles.moveLater", { field: label })}
                  title={t("titles.moveLater", { field: label })}
                >
                  <ChevronRight className="h-3.5 w-3.5" />
                </button>
                <button
                  className="rounded-r-md border-l border-slate-300 px-1 py-1 text-slate-400 hover:bg-slate-200 hover:text-slate-700 dark:border-slate-700 dark:hover:bg-slate-700 dark:hover:text-slate-200"
                  onClick={() => setWords(words.filter((_, i) => i !== at))}
                  aria-label={t("titles.removePart", { field: label })}
                  title={t("titles.removePart", { field: label })}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </span>
            );
          })}
          {unused.length > 0 && (
            <Select<TitleField | "">
              className="w-44"
              value=""
              placeholder={t("titles.addPart")}
              options={unused.map((f) => ({ value: f, label: titleFieldLabel(f) }))}
              onChange={(f) => f && setWords([...words, placeholder(f)])}
              aria-label={t("titles.addPart")}
            />
          )}
          <input
            className="input w-36 py-1 text-sm"
            value={text}
            placeholder={t("titles.addText")}
            aria-label={t("titles.addText")}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addText()}
            onBlur={addText}
          />
        </div>
        {words.length === 0 && <p className="mt-1.5 text-xs text-slate-500">{t("titles.noParts")}</p>}
      </div>
      <div>
        <label className="label" htmlFor={`pattern-${tpl.id}`}>
          {t("titles.pattern")}
        </label>
        <input
          id={`pattern-${tpl.id}`}
          className="input font-mono text-sm"
          value={pattern}
          spellCheck={false}
          onChange={(e) => set(e.target.value)}
        />
        <p className="mt-1 text-xs text-slate-500">
          {t("titles.patternHint", { parts: TITLE_FIELDS.map((f) => `{${f}}`).join(" ") })}
        </p>
        {unknown.length > 0 && (
          <p className="mt-1 text-xs text-amber-700 dark:text-amber-400">
            {t("titles.unknownPart", { names: unknown.map((k) => `{${k}}`).join(", ") })}
          </p>
        )}
      </div>
    </div>
  );
}

function PreviewRow({ row }: { row: TitlePreview }) {
  const t = useT();
  const leftOut = leftOutText(row.leftOut);
  const note = row.problem
    ? row.problem
    : row.custom
      ? t("titles.preview.custom")
      : [row.template || t("titles.preview.builtin"), leftOut].filter(Boolean).join(" · ");
  return (
    <li className="flex items-center gap-3 px-3 py-2">
      {row.thumbPath ? (
        <img src={fileUrl(row.thumbPath)} alt="" className="h-9 w-auto max-w-[27px] rounded object-contain" />
      ) : (
        <span className="h-9 w-[27px] shrink-0 rounded bg-slate-100 dark:bg-slate-800" />
      )}
      <div className="min-w-0 flex-1">
        <div className="selectable text-sm font-medium break-words">{row.title || "—"}</div>
        <div className="truncate text-xs text-slate-500" title={note}>
          {note}
        </div>
      </div>
      {row.title && <TitleCount text={row.title} />}
    </li>
  );
}
