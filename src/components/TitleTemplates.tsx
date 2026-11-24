// Settings › Selling › Listing titles: the user's title templates, each a list of parts in order for the cards it is
// for, with the titles they give shown as they are edited.

import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Plus, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { t as translate, useT } from "../i18n";
import { api, type TitleField, type TitlePreview, type TitleTemplate } from "../lib/api";
import { fileUrl } from "../lib/format";
import Select, { type SelectOption } from "./Select";
import { leftOutText, TITLE_FIELDS, TitleCount, titleFieldLabel } from "./TitleParts";

/** What a new template starts with: a sports title with what the built-in one leaves out. */
const STARTING_FIELDS: TitleField[] = [
  "year",
  "brand",
  "set",
  "name",
  "series",
  "features",
  "serial",
  "number",
  "grade",
];

function kindOptions(): SelectOption<TitleTemplate["kind"]>[] {
  return [
    { value: "any", label: translate("titles.kind.any") },
    { value: "sport", label: translate("titles.kind.sport") },
    { value: "tcg", label: translate("titles.kind.tcg") },
  ];
}

function onlyOptions(): SelectOption<TitleTemplate["only"]>[] {
  return [
    { value: "any", label: translate("titles.only.any") },
    { value: "graded", label: translate("titles.only.graded"), hint: translate("titles.only.gradedHint") },
    { value: "autograph", label: translate("titles.only.autograph"), hint: translate("titles.only.autographHint") },
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
        fields: STARTING_FIELDS,
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
  const unused = TITLE_FIELDS.filter((f) => !tpl.fields.includes(f));
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

      <div>
        <div className="label">{t("titles.parts")}</div>
        <div className="flex flex-wrap items-center gap-1.5">
          {tpl.fields.map((field, at) => {
            const label = TITLE_FIELDS.includes(field) ? titleFieldLabel(field) : field;
            return (
              <span
                key={field}
                className="inline-flex items-center rounded-md border border-slate-300 bg-slate-50 text-sm dark:border-slate-700 dark:bg-slate-800"
              >
                <button
                  className="rounded-l-md px-0.5 py-1 text-slate-400 hover:bg-slate-200 hover:text-slate-700 disabled:opacity-30 dark:hover:bg-slate-700 dark:hover:text-slate-200"
                  disabled={at === 0}
                  onClick={() => onChange({ fields: moved(tpl.fields, at, -1) })}
                  aria-label={t("titles.moveEarlier", { field: label })}
                  title={t("titles.moveEarlier", { field: label })}
                >
                  <ChevronLeft className="h-3.5 w-3.5" />
                </button>
                <span className="px-1 font-medium">{label}</span>
                <button
                  className="px-0.5 py-1 text-slate-400 hover:bg-slate-200 hover:text-slate-700 disabled:opacity-30 dark:hover:bg-slate-700 dark:hover:text-slate-200"
                  disabled={at === tpl.fields.length - 1}
                  onClick={() => onChange({ fields: moved(tpl.fields, at, 1) })}
                  aria-label={t("titles.moveLater", { field: label })}
                  title={t("titles.moveLater", { field: label })}
                >
                  <ChevronRight className="h-3.5 w-3.5" />
                </button>
                <button
                  className="rounded-r-md border-l border-slate-300 px-1 py-1 text-slate-400 hover:bg-slate-200 hover:text-slate-700 dark:border-slate-700 dark:hover:bg-slate-700 dark:hover:text-slate-200"
                  onClick={() => onChange({ fields: tpl.fields.filter((f) => f !== field) })}
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
              onChange={(f) => f && onChange({ fields: [...tpl.fields, f] })}
              aria-label={t("titles.addPart")}
            />
          )}
        </div>
        {tpl.fields.length === 0 && <p className="mt-1.5 text-xs text-slate-500">{t("titles.noParts")}</p>}
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
