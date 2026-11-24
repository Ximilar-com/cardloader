// One text for the ticked cards: at the start or end of their listing titles, or in one of their fields. Every
// card's title is shown as it would become before anything is changed.

import clsx from "clsx";
import { ArrowRight } from "lucide-react";
import { useEffect, useState } from "react";
import { t as translate, useT } from "../i18n";
import { api, errorText, MAX_TITLE, type BulkEdit, type TitlePreview } from "../lib/api";
import { fileUrl } from "../lib/format";
import Modal, { ErrorNote } from "./Modal";
import Select, { type SelectOption } from "./Select";
import { CustomTitleMark, TitleCount } from "./TitleParts";
import { toast } from "./Toast";

function fieldOptions(): SelectOption<BulkEdit["field"]>[] {
  return [
    { value: "title", label: translate("titles.bulk.field.title") },
    { value: "features", label: translate("status.field.features") },
    { value: "series", label: translate("status.field.series") },
    { value: "brand", label: translate("status.field.brand") },
    { value: "team", label: translate("status.field.team") },
  ];
}

function placeOptions(): SelectOption<BulkEdit["place"]>[] {
  return [
    { value: "end", label: translate("titles.bulk.place.end") },
    { value: "start", label: translate("titles.bulk.place.start") },
    { value: "replace", label: translate("titles.bulk.place.replace") },
  ];
}

export default function BulkEditDialog({
  itemIds,
  onClose,
  onDone,
}: {
  itemIds: number[];
  onClose: () => void;
  /** The cards were changed (the dialog then closes). */
  onDone?: () => void;
}) {
  const t = useT();
  const [edit, setEdit] = useState<BulkEdit>({ field: "title", place: "end", text: "" });
  /** The titles last read, and the change they were read for (while typing, they are a moment behind). */
  const [read, setRead] = useState<{ key: string; titles: TitlePreview[] } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Nothing to add yet: the titles are shown as they are. Replacing with nothing is a change (it empties). */
  const asked = edit.text.trim() !== "" || edit.place === "replace";
  const key = asked ? JSON.stringify(edit) : "";

  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => {
      api
        .previewTitles({ itemIds, edit: key ? (JSON.parse(key) as BulkEdit) : undefined })
        .then((titles) => live && setRead({ key, titles }))
        .catch((e) => live && setError(errorText(e)));
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [itemIds, key]);

  const shown = read?.titles ?? null;
  /** The titles shown are the ones this change gives: only then do they say what will change. */
  const current = asked && read?.key === key;
  const changing = current ? (shown?.filter((row) => row.changed).length ?? 0) : 0;
  const refused = current ? (shown?.filter((row) => row.problem).length ?? 0) : 0;
  const run = () => {
    setBusy(true);
    setError(null);
    api
      .editItems(itemIds, edit)
      .then(({ changed, skipped }) => {
        toast(
          skipped > 0
            ? t("titles.bulk.toastSkipped", { count: changed, skipped })
            : t("titles.bulk.toast", { count: changed }),
        );
        onDone?.();
        onClose();
      })
      .catch((e) => {
        setError(errorText(e));
        setBusy(false);
      });
  };
  const hint =
    edit.field === "title"
      ? edit.place === "replace"
        ? t("titles.bulk.hint.titleReplace", { max: MAX_TITLE })
        : t("titles.bulk.hint.title", { max: MAX_TITLE })
      : edit.field === "series"
        ? t("titles.bulk.hint.series")
        : t("titles.bulk.hint.field");

  return (
    <Modal
      title={t("titles.bulk.title", { count: itemIds.length })}
      onClose={onClose}
      width="max-w-3xl"
      footer={
        <>
          {current && (
            <span className="mr-auto self-center text-sm text-slate-500">
              {refused > 0
                ? t("titles.bulk.summarySkipped", { count: changing, skipped: refused })
                : t("titles.bulk.summary", { count: changing })}
            </span>
          )}
          <button className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button className="btn btn-primary" disabled={busy || changing === 0} onClick={run}>
            {current ? t("titles.bulk.apply", { count: changing }) : t("titles.bulk.applyNone")}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid grid-cols-[1fr_1fr_2fr] gap-3">
          <div>
            <div className="label">{t("titles.bulk.field.label")}</div>
            <Select
              value={edit.field}
              options={fieldOptions()}
              onChange={(field) => setEdit((e) => ({ ...e, field }))}
              aria-label={t("titles.bulk.field.label")}
            />
          </div>
          <div>
            <div className="label">{t("titles.bulk.place.label")}</div>
            <Select
              value={edit.place}
              options={placeOptions()}
              onChange={(place) => setEdit((e) => ({ ...e, place }))}
              aria-label={t("titles.bulk.place.label")}
            />
          </div>
          <div>
            <label className="label" htmlFor="bulk-edit-text">
              {t("titles.bulk.text")}
            </label>
            <input
              id="bulk-edit-text"
              className="input"
              autoFocus
              value={edit.text}
              placeholder={t("titles.bulk.textPlaceholder")}
              onChange={(e) => setEdit((x) => ({ ...x, text: e.target.value }))}
              onKeyDown={(e) => e.key === "Enter" && changing > 0 && !busy && run()}
            />
          </div>
        </div>
        <p className="text-sm text-slate-500">{hint}</p>

        <ul className="max-h-[45vh] divide-y divide-slate-100 overflow-y-auto rounded-md border border-slate-200 dark:divide-slate-800 dark:border-slate-800">
          {(shown ?? []).map((row) => (
            <Row key={row.itemId} row={row} asked={current} />
          ))}
        </ul>
        <ErrorNote>{error}</ErrorNote>
      </div>
    </Modal>
  );
}

function Row({ row, asked }: { row: TitlePreview; asked: boolean }) {
  const t = useT();
  const note = row.problem ?? (asked && !row.changed ? t("titles.bulk.unchanged") : null);
  return (
    <li className={clsx("flex items-center gap-3 px-3 py-2", asked && !row.changed && "opacity-60")}>
      {row.thumbPath ? (
        <img src={fileUrl(row.thumbPath)} alt="" className="h-10 w-auto max-w-[30px] rounded object-contain" />
      ) : (
        <span className="h-10 w-[30px] shrink-0 rounded bg-slate-100 dark:bg-slate-800" />
      )}
      <span className="w-6 shrink-0 text-xs text-slate-400 tabular-nums">{row.seq}</span>
      <div className="min-w-0 flex-1 text-sm">
        {row.changed && row.before !== row.title && (
          <div className="truncate text-xs text-slate-400 line-through" title={row.before}>
            {row.before}
          </div>
        )}
        <div className="selectable flex items-start gap-1.5 font-medium">
          {row.changed && row.before !== row.title && (
            <ArrowRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-slate-400" />
          )}
          <span className="break-words">{row.title || "—"}</span>
        </div>
        {note && <div className={clsx("text-xs", row.problem ? "text-red-600" : "text-slate-500")}>{note}</div>}
      </div>
      {row.custom && <CustomTitleMark />}
      {row.title && <TitleCount text={row.title} />}
    </li>
  );
}
