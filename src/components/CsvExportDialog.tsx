// "Export CSV…": where the file goes and which columns it has, in what order. Both are remembered for the next
// export (`AppState::export_csv`).

import { save } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import { Check, ChevronLeft, ChevronRight, FileDown, Plus, RotateCcw, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useT } from "../i18n";
import { api, errorText, type CsvColumn, type CsvHeaders, type Session } from "../lib/api";
import { CSV_COLUMNS, csvColumnsOf, csvHeader, renamedTo, repeatedHeaders } from "../lib/csvColumns";
import Modal, { ErrorNote } from "./Modal";
import { toast } from "./Toast";

/** The file name a session's CSV gets: its name, without characters a file name can't have. */
function fileName(s: Session): string {
  return `${s.name.replace(/[\\/:*?"<>|]+/g, "-").trim() || "cards"}.csv`;
}

/** `name` in `folder`, with the folder's own separator. */
function inFolder(folder: string, name: string): string {
  const sep = folder.includes("\\") && !folder.includes("/") ? "\\" : "/";
  return folder.endsWith(sep) ? folder + name : folder + sep + name;
}

/** `list` with the item at `from` moved by `by` places; unchanged at an end. */
function moved<T>(list: T[], from: number, by: number): T[] {
  const to = from + by;
  if (to < 0 || to >= list.length) return list;
  const out = [...list];
  out.splice(to, 0, ...out.splice(from, 1));
  return out;
}

export default function CsvExportDialog({ session, onClose }: { session: Session; onClose: () => void }) {
  const t = useT();
  const [path, setPath] = useState("");
  const [columns, setColumns] = useState<CsvColumn[] | null>(null);
  const [headers, setHeaders] = useState<CsvHeaders>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Where the last export went and the columns it had.
  useEffect(() => {
    let live = true;
    api
      .getSettings()
      .then((s) => {
        if (!live) return;
        setColumns(csvColumnsOf(s.csvColumns));
        setHeaders(s.csvHeaders ?? {});
        if (s.csvFolder) setPath((p) => p || inFolder(s.csvFolder!, fileName(session)));
      })
      .catch(() => live && setColumns([...CSV_COLUMNS]));
    return () => {
      live = false;
    };
  }, [session]);

  const choose = async () => {
    try {
      const picked = await save({
        defaultPath: path || fileName(session),
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (picked) setPath(picked);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const run = async () => {
    if (!columns?.length || !path.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const n = await api.exportCsv(session.id, path.trim(), columns, headers);
      toast(t("session.actions.exported", { count: n, path: path.trim() }));
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  const chosen = columns ?? [];
  const left = CSV_COLUMNS.filter((c) => !chosen.includes(c));
  const repeated = repeatedHeaders(chosen, headers);
  /** "name: primary name" for a renamed column, else its header. */
  const label = (c: CsvColumn) => (headers[c] ? `${csvHeader(c)}: ${csvHeader(c, headers)}` : csvHeader(c));
  const chip =
    "inline-flex items-center rounded-md border text-sm font-mono border-slate-300 bg-slate-50 dark:border-slate-700 dark:bg-slate-800";
  const arrow =
    "px-0.5 py-1 text-slate-400 hover:bg-slate-200 hover:text-slate-700 disabled:opacity-30 dark:hover:bg-slate-700 dark:hover:text-slate-200";

  return (
    <Modal
      title={t("session.csvExport.title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button
            className="btn btn-primary"
            disabled={busy || !chosen.length || !path.trim()}
            onClick={() => void run()}
          >
            <FileDown className="h-4 w-4" /> {t("session.csvExport.export", { count: session.counts.total })}
          </button>
        </>
      }
    >
      <div className="space-y-5">
        <div>
          <label className="label" htmlFor="csv-path">
            {t("session.csvExport.saveTo")}
          </label>
          <div className="flex gap-2">
            <input
              id="csv-path"
              className="input min-w-0 flex-1 font-mono text-sm"
              value={path}
              placeholder={t("session.csvExport.pathPlaceholder")}
              spellCheck={false}
              onChange={(e) => setPath(e.target.value)}
            />
            <button className="btn shrink-0" onClick={() => void choose()}>
              {t("common.browse")}
            </button>
          </div>
        </div>

        <div>
          <div className="mb-1 flex items-end justify-between gap-2">
            <div className="label mb-0">{t("session.csvExport.columns", { count: chosen.length })}</div>
            <div className="flex gap-1">
              <button
                className="btn btn-ghost px-2 py-0.5 text-xs"
                disabled={chosen.length === CSV_COLUMNS.length && chosen.every((c, i) => c === CSV_COLUMNS[i])}
                onClick={() => setColumns([...CSV_COLUMNS])}
              >
                <RotateCcw className="h-3.5 w-3.5" /> {t("session.csvExport.reset")}
              </button>
              <button
                className="btn btn-ghost px-2 py-0.5 text-xs"
                disabled={!chosen.length}
                onClick={() => setColumns([])}
              >
                <X className="h-3.5 w-3.5" /> {t("session.csvExport.none")}
              </button>
            </div>
          </div>
          <p className="mb-2 text-xs text-slate-500">{t("session.csvExport.columnsHint")}</p>
          <div className="flex flex-wrap gap-1.5">
            {chosen.map((column, at) => {
              const name = csvHeader(column);
              return (
                <span key={column} className={chip}>
                  <button
                    className={clsx(arrow, "rounded-l-md")}
                    disabled={at === 0}
                    onClick={() => setColumns(moved(chosen, at, -1))}
                    aria-label={t("titles.moveEarlier", { field: name })}
                    title={t("titles.moveEarlier", { field: name })}
                  >
                    <ChevronLeft className="h-3.5 w-3.5" />
                  </button>
                  <ColumnName
                    column={column}
                    label={label(column)}
                    renamed={headers[column]}
                    onRename={(to) => setHeaders((h) => renamedTo(h, column, to))}
                  />
                  <button
                    className={arrow}
                    disabled={at === chosen.length - 1}
                    onClick={() => setColumns(moved(chosen, at, 1))}
                    aria-label={t("titles.moveLater", { field: name })}
                    title={t("titles.moveLater", { field: name })}
                  >
                    <ChevronRight className="h-3.5 w-3.5" />
                  </button>
                  <button
                    className={clsx(arrow, "rounded-r-md border-l border-slate-300 px-1 dark:border-slate-700")}
                    onClick={() => setColumns(chosen.filter((c) => c !== column))}
                    aria-label={t("session.csvExport.leaveOut", { column: name })}
                    title={t("session.csvExport.leaveOut", { column: name })}
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                </span>
              );
            })}
            {columns && !chosen.length && (
              <p className="text-sm text-amber-700 dark:text-amber-400">{t("session.csvExport.noColumns")}</p>
            )}
          </div>
          {repeated.length > 0 && (
            <p className="mt-2 text-xs text-amber-700 dark:text-amber-400">
              {t("session.csvExport.repeated", { headers: repeated.map((h) => `“${h}”`).join(", ") })}
            </p>
          )}
        </div>

        {left.length > 0 && (
          <div>
            <div className="label">{t("session.csvExport.leftOut")}</div>
            <div className="flex flex-wrap gap-1.5">
              {left.map((column) => (
                <button
                  key={column}
                  className="inline-flex items-center gap-1 rounded-md border border-dashed border-slate-300 px-1.5 py-1 font-mono text-sm text-slate-500 hover:border-slate-400 hover:bg-slate-50 hover:text-slate-800 dark:border-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-200"
                  onClick={() => setColumns([...chosen, column])}
                  title={t("session.csvExport.add", { column: csvHeader(column) })}
                >
                  <Plus className="h-3.5 w-3.5" /> {label(column)}
                </button>
              ))}
            </div>
          </div>
        )}

        {chosen.length > 0 && (
          <div>
            <div className="label">{t("session.csvExport.header")}</div>
            <code className="selectable block overflow-x-auto rounded-md bg-slate-50 px-2 py-1.5 text-xs whitespace-nowrap text-slate-600 dark:bg-slate-900 dark:text-slate-300">
              {chosen.map((c) => csvHeader(c, headers)).join(",")}
            </code>
          </div>
        )}

        <ErrorNote>{error}</ErrorNote>
      </div>
    </Modal>
  );
}

/** A column's name on its badge; clicked, a field for the header the file gets instead (empty = its own). */
function ColumnName({
  column,
  label,
  renamed,
  onRename,
}: {
  column: CsvColumn;
  label: string;
  renamed?: string;
  onRename: (to: string) => void;
}) {
  const t = useT();
  const [draft, setDraft] = useState<string | null>(null);
  // Escape closes the field without its blur keeping what was typed.
  const cancelled = useRef(false);
  const own = csvHeader(column);
  const done = (keep: boolean) => {
    if (draft != null && keep && !cancelled.current) onRename(draft);
    setDraft(null);
  };

  if (draft == null)
    return (
      <button
        className="px-1 py-0.5 hover:bg-slate-200 dark:hover:bg-slate-700"
        onClick={() => {
          cancelled.current = false;
          setDraft(renamed ?? "");
        }}
        title={t("session.csvExport.rename", { column: own })}
      >
        {renamed ? (
          <>
            {own}: <span className="font-sans font-medium text-sky-700 dark:text-sky-400">{renamed}</span>
          </>
        ) : (
          label
        )}
      </button>
    );

  return (
    <span className="inline-flex items-center gap-1 px-1">
      <span>{own}:</span>
      <input
        className="input w-36 px-1.5 py-0.5 font-sans text-sm"
        value={draft}
        autoFocus
        placeholder={own}
        aria-label={t("session.csvExport.rename", { column: own })}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") done(true);
          else if (e.key === "Escape") {
            cancelled.current = true;
            done(false);
          } else return;
          // Not the dialog's: Escape would close it, Enter would export.
          e.preventDefault();
          e.stopPropagation();
        }}
        onBlur={() => done(true)}
      />
      <button
        className="rounded p-0.5 text-emerald-700 hover:bg-emerald-50 dark:text-emerald-400 dark:hover:bg-emerald-950"
        // Before the field's blur.
        onMouseDown={(e) => {
          e.preventDefault();
          done(true);
        }}
        title={t("session.csvExport.renameOk")}
        aria-label={t("session.csvExport.renameOk")}
      >
        <Check className="h-3.5 w-3.5" />
      </button>
    </span>
  );
}
