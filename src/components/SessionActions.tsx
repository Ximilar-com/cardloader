// Managing local sessions: rename, settings, duplicate, show files, export, delete. Shared by the
// sessions list (row menu, bulk delete) and the session page (header menu).

import { save } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { Copy, Cpu, FileDown, FileSpreadsheet, FolderOpen, Pencil, Settings2, Store, Trash2 } from "lucide-react";
import { useCallback, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { api, errorText, type ExportFormat, type RemoteSession, type Session } from "../lib/api";
import type { MenuItem } from "./Menu";
import Modal, { ErrorNote } from "./Modal";
import { Check } from "./NewSessionDialog";
import CompareDialog from "./CompareDialog";
import { EbayListDialog } from "./Ebay";
import { MarketExportDialog } from "./MarketExport";
import NewSessionDialog from "./NewSessionDialog";
import { toast, toastError as fail } from "./Toast";

type Dialog =
  | { kind: "rename"; session: Session }
  | { kind: "settings"; session: Session }
  | { kind: "duplicate"; session: Session }
  | { kind: "compare"; session: Session }
  | { kind: "ebay"; session: Session }
  | { kind: "market"; session: Session; format: ExportFormat }
  | { kind: "delete"; sessions: Session[] };

const isActive = (s: Session) => s.status === "running" || s.status === "paused";

export function useSessionActions({ onDeleted }: { onDeleted?: (ids: string[]) => void } = {}) {
  const t = useT();
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const close = useCallback(() => setDialog(null), []);

  const showFiles = (s: Session) =>
    api
      .sessionFolder(s.id)
      .then((path) => openPath(path))
      .catch(fail);

  const exportCsv = async (s: Session) => {
    try {
      const path = await save({
        defaultPath: `${s.name.replace(/[\\/:*?"<>|]+/g, "-")}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!path) return;
      const n = await api.exportCsv(s.id, path);
      toast(t("session.actions.exported", { count: n, path }));
    } catch (e) {
      fail(e);
    }
  };

  /** Files and marketplaces the session's cards can go to. */
  const sellMenu = (s: Session): MenuItem[] => [
    {
      label: t("session.actions.listOnEbay"),
      icon: Store,
      onClick: () => setDialog({ kind: "ebay", session: s }),
      disabled: isActive(s) || s.counts.done === 0,
      title: isActive(s) ? t("session.actions.waitUntilFinished") : t("session.actions.listOnEbayTitle"),
    },
    ...(["tcgplayer", "whatnot", "shopify"] as const).map((format) => ({
      label: t("session.actions.exportFor", {
        market: { tcgplayer: "TCGplayer", whatnot: "Whatnot", shopify: "Shopify" }[format],
      }),
      icon: FileSpreadsheet,
      onClick: () => setDialog({ kind: "market", session: s, format }),
      disabled: s.counts.done === 0,
    })),
    {
      label: t("session.actions.exportCsv"),
      icon: FileDown,
      onClick: () => void exportCsv(s),
      disabled: s.counts.total === 0,
    },
  ];

  /** `sell: false` leaves out what `sellMenu` has, for a page that shows it on its own. */
  const menu = (s: Session, { open, sell = true }: { open?: () => void; sell?: boolean } = {}): MenuItem[] =>
    [
      open && { label: t("common.open"), icon: FolderOpen, onClick: open },
      { label: t("session.actions.rename"), icon: Pencil, onClick: () => setDialog({ kind: "rename", session: s }) },
      {
        label: t("session.actions.settings"),
        icon: Settings2,
        onClick: () => setDialog({ kind: "settings", session: s }),
        disabled: isActive(s),
        title: isActive(s) ? t("session.actions.stopToChangeSettings") : undefined,
      },
      {
        label: t("session.actions.duplicate"),
        icon: Copy,
        onClick: () => setDialog({ kind: "duplicate", session: s }),
      },
      {
        label: s.sourceKind === "scanner" ? t("session.actions.showScans") : t("session.actions.showFolder"),
        icon: FolderOpen,
        onClick: () => void showFiles(s),
      },
      s.options.engine !== "local" && {
        label: t("session.actions.compare"),
        icon: Cpu,
        onClick: () => setDialog({ kind: "compare", session: s }),
        disabled: s.counts.done === 0,
        title: t("session.actions.compareTitle"),
      },
      ...(sell ? sellMenu(s) : []),
      {
        label: t("session.actions.delete"),
        icon: Trash2,
        danger: true,
        onClick: () => setDialog({ kind: "delete", sessions: [s] }),
      },
    ].filter(Boolean) as MenuItem[];

  const node: ReactNode = (
    <>
      {dialog?.kind === "rename" && (
        <RenameDialog
          title={t("session.actions.renameTitle")}
          initial={dialog.session.name}
          hint={dialog.session.remoteSessionId ? t("session.actions.renameHint") : undefined}
          onClose={close}
          onSave={(name) => api.renameSession(dialog.session.id, name)}
        />
      )}
      {dialog?.kind === "settings" && <NewSessionDialog mode="edit" from={dialog.session} onClose={close} />}
      {dialog?.kind === "duplicate" && <NewSessionDialog mode="duplicate" from={dialog.session} onClose={close} />}
      {dialog?.kind === "compare" && <CompareDialog session={dialog.session} onClose={close} />}
      {dialog?.kind === "ebay" && <EbayListDialog session={dialog.session} onClose={close} />}
      {dialog?.kind === "market" && (
        <MarketExportDialog key={dialog.format} session={dialog.session} format={dialog.format} onClose={close} />
      )}
      {dialog?.kind === "delete" && (
        <DeleteSessionsDialog sessions={dialog.sessions} onClose={close} onDeleted={(ids) => onDeleted?.(ids)} />
      )}
    </>
  );

  return {
    menu,
    sellMenu,
    node,
    rename: (s: Session) => setDialog({ kind: "rename", session: s }),
    remove: (sessions: Session[]) => sessions.length && setDialog({ kind: "delete", sessions }),
  };
}

export function RenameDialog({
  title,
  initial,
  hint,
  confirm,
  onClose,
  onSave,
}: {
  title: string;
  initial: string;
  hint?: string;
  confirm?: string;
  onClose: () => void;
  onSave: (name: string) => Promise<unknown>;
}) {
  const t = useT();
  const [name, setName] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = () => {
    if (name.trim() === initial.trim()) return onClose();
    setBusy(true);
    setError(null);
    onSave(name)
      .then(onClose)
      .catch((e) => {
        setError(errorText(e));
        setBusy(false);
      });
  };
  return (
    <Modal
      title={title}
      width="max-w-md"
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button className="btn btn-primary" onClick={submit} disabled={busy || !name.trim()}>
            {confirm ?? t("session.actions.renameConfirm")}
          </button>
        </>
      }
    >
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <input
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
          onFocus={(e) => e.target.select()}
        />
        {hint && <p className="text-xs text-slate-500">{hint}</p>}
        <ErrorNote>{error}</ErrorNote>
      </form>
    </Modal>
  );
}

function DeleteSessionsDialog({
  sessions,
  onClose,
  onDeleted,
}: {
  sessions: Session[];
  onClose: () => void;
  onDeleted: (ids: string[]) => void;
}) {
  const t = useT();
  const [withRemote, setWithRemote] = useState(false);
  const [busy, setBusy] = useState(false);
  const [errors, setErrors] = useState<string[]>([]);
  const one = sessions.length === 1;
  const cards = sessions.reduce((n, s) => n + s.counts.total, 0);
  const remote = sessions.filter((s) => s.remoteSessionId).length;
  const scanned = sessions.some((s) => s.sourceKind === "scanner");
  const running = sessions.some(isActive);

  const run = async () => {
    setBusy(true);
    const deleted: string[] = [];
    const failed: string[] = [];
    for (const s of sessions) {
      try {
        await api.deleteSession(s.id, withRemote);
        deleted.push(s.id);
      } catch (e) {
        failed.push(`${s.name}: ${errorText(e)}`);
      }
    }
    if (deleted.length) onDeleted(deleted);
    if (failed.length) {
      setErrors(failed);
      setBusy(false);
    } else {
      onClose();
    }
  };

  return (
    <Modal
      title={
        one ? t("session.deleteSessions.titleOne") : t("session.deleteSessions.titleMany", { count: sessions.length })
      }
      width="max-w-md"
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button className="btn btn-danger" onClick={run} disabled={busy}>
            {busy ? t("session.deleteSessions.deleting") : t("common.delete")}
          </button>
        </>
      }
    >
      <div className="space-y-3 text-sm text-slate-600 dark:text-slate-300">
        {one ? (
          <p>{t("session.deleteSessions.textOne", { name: sessions[0].name, count: cards })}</p>
        ) : (
          <>
            <p>{t("session.deleteSessions.textMany", { count: cards })}</p>
            <ul className="max-h-40 list-inside list-disc overflow-y-auto">
              {sessions.map((s) => (
                <li key={s.id} className="truncate">
                  {s.name}
                </li>
              ))}
            </ul>
          </>
        )}
        <p>
          {t("session.deleteSessions.thumbnails")}{" "}
          {scanned ? t("session.deleteSessions.scanned") : t("session.deleteSessions.originals")}
        </p>
        {running && <p className="text-amber-700 dark:text-amber-400">{t("session.deleteSessions.runningStopped")}</p>}
        {remote > 0 && (
          <div className="rounded-md border border-slate-200 p-3 dark:border-slate-800">
            <Check
              label={
                one
                  ? t("session.deleteSessions.alsoRemoteOne")
                  : t("session.deleteSessions.alsoRemoteMany", { count: remote })
              }
              checked={withRemote}
              onChange={setWithRemote}
            />
            <p className="mt-1 pl-6 text-xs text-slate-500">
              {withRemote ? t("session.deleteSessions.remoteDeleted") : t("session.deleteSessions.remoteKept")}
            </p>
          </div>
        )}
        {errors.length > 0 && (
          <ErrorNote>
            {errors.map((e) => (
              <div key={e}>{e}</div>
            ))}
          </ErrorNote>
        )}
      </div>
    </Modal>
  );
}

/** Rename and delete for sessions in Cards Cloud (the Cards Cloud tab). */
export function useRemoteSessionActions({
  onRenamed,
  onDeleted,
}: {
  onRenamed?: (s: RemoteSession) => void;
  onDeleted?: (id: string) => void;
}) {
  const t = useT();
  const [dialog, setDialog] = useState<{ kind: "rename" | "delete"; session: RemoteSession } | null>(null);
  const close = useCallback(() => setDialog(null), []);

  const menu = (s: RemoteSession, { open }: { open?: () => void } = {}): MenuItem[] =>
    [
      open && { label: t("common.open"), icon: FolderOpen, onClick: open },
      { label: t("session.actions.rename"), icon: Pencil, onClick: () => setDialog({ kind: "rename", session: s }) },
      {
        label: t("session.actions.delete"),
        icon: Trash2,
        danger: true,
        onClick: () => setDialog({ kind: "delete", session: s }),
      },
    ].filter(Boolean) as MenuItem[];

  const node: ReactNode = (
    <>
      {dialog?.kind === "rename" && (
        <RenameDialog
          title={t("session.remote.renameTitle")}
          initial={dialog.session.name}
          onClose={close}
          onSave={(name) => api.renameRemoteSession(dialog.session.id, name).then((s) => onRenamed?.(s))}
        />
      )}
      {dialog?.kind === "delete" && (
        <ConfirmDialog
          title={t("session.remote.deleteTitle")}
          confirm={t("common.delete")}
          onClose={close}
          onConfirm={() => api.deleteRemoteSession(dialog.session.id).then(() => onDeleted?.(dialog.session.id))}
        >
          {t("session.remote.deleteText", {
            name: dialog.session.name || t("session.remote.untitled"),
            count: dialog.session.identification_count,
          })}
        </ConfirmDialog>
      )}
    </>
  );

  return { menu, node };
}

export function ConfirmDialog({
  title,
  confirm,
  children,
  onClose,
  onConfirm,
  danger = true,
}: {
  title: string;
  confirm: string;
  children: ReactNode;
  /** A red confirm button; false for an ordinary action. */
  danger?: boolean;
  onClose: () => void;
  onConfirm: () => Promise<unknown>;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <Modal
      title={title}
      width="max-w-md"
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button
            className={danger ? "btn btn-danger" : "btn btn-primary"}
            disabled={busy}
            onClick={() => {
              setBusy(true);
              setError(null);
              onConfirm()
                .then(onClose)
                .catch((e) => {
                  setError(errorText(e));
                  setBusy(false);
                });
            }}
          >
            {confirm}
          </button>
        </>
      }
    >
      <div className="space-y-3 text-sm text-slate-600 dark:text-slate-300">
        <p>{children}</p>
        <ErrorNote>{error}</ErrorNote>
      </div>
    </Modal>
  );
}
