// Managing local sessions: rename, settings, duplicate, show files, export, delete. Shared by the
// sessions list (row menu, bulk delete) and the session page (header menu).

import { save } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { Copy, Cpu, FileDown, FolderOpen, Pencil, Settings2, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, errorText, type RemoteSession, type Session } from "../lib/api";
import type { MenuItem } from "./Menu";
import Modal, { ErrorNote } from "./Modal";
import { Check } from "./NewSessionDialog";
import CompareDialog from "./CompareDialog";
import NewSessionDialog from "./NewSessionDialog";

type Dialog =
  | { kind: "rename"; session: Session }
  | { kind: "settings"; session: Session }
  | { kind: "duplicate"; session: Session }
  | { kind: "compare"; session: Session }
  | { kind: "delete"; sessions: Session[] };

const isActive = (s: Session) => s.status === "running" || s.status === "paused";

export function useSessionActions({ onDeleted }: { onDeleted?: (ids: string[]) => void } = {}) {
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const close = useCallback(() => setDialog(null), []);

  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), notice.error ? 8000 : 4000);
    return () => clearTimeout(t);
  }, [notice]);

  const fail = (e: unknown) => setNotice({ text: errorText(e), error: true });

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
      setNotice({ text: `Exported ${n} card${n === 1 ? "" : "s"} to ${path}` });
    } catch (e) {
      fail(e);
    }
  };

  const menu = (s: Session, { open }: { open?: () => void } = {}): MenuItem[] =>
    [
      open && { label: "Open", icon: FolderOpen, onClick: open },
      { label: "Rename…", icon: Pencil, onClick: () => setDialog({ kind: "rename", session: s }) },
      {
        label: "Settings…",
        icon: Settings2,
        onClick: () => setDialog({ kind: "settings", session: s }),
        disabled: isActive(s),
        title: isActive(s) ? "Stop the session to change its settings" : undefined,
      },
      { label: "Duplicate…", icon: Copy, onClick: () => setDialog({ kind: "duplicate", session: s }) },
      {
        label: s.sourceKind === "scanner" ? "Show scans" : "Show folder",
        icon: FolderOpen,
        onClick: () => void showFiles(s),
      },
      {
        label: "Export CSV…",
        icon: FileDown,
        onClick: () => void exportCsv(s),
        disabled: s.counts.total === 0,
      },
      s.options.engine !== "local" && {
        label: "Compare with local model…",
        icon: Cpu,
        onClick: () => setDialog({ kind: "compare", session: s }),
        disabled: s.counts.done === 0,
        title: "How often the on-device model reads the same card as Ximilar",
      },
      { label: "Delete…", icon: Trash2, danger: true, onClick: () => setDialog({ kind: "delete", sessions: [s] }) },
    ].filter(Boolean) as MenuItem[];

  const node: ReactNode = (
    <>
      {dialog?.kind === "rename" && (
        <RenameDialog
          title="Rename session"
          initial={dialog.session.name}
          hint={dialog.session.remoteSessionId ? "The Cards Cloud copy is renamed too." : undefined}
          onClose={close}
          onSave={(name) => api.renameSession(dialog.session.id, name)}
        />
      )}
      {dialog?.kind === "settings" && <NewSessionDialog mode="edit" from={dialog.session} onClose={close} />}
      {dialog?.kind === "duplicate" && <NewSessionDialog mode="duplicate" from={dialog.session} onClose={close} />}
      {dialog?.kind === "compare" && <CompareDialog session={dialog.session} onClose={close} />}
      {dialog?.kind === "delete" && (
        <DeleteSessionsDialog sessions={dialog.sessions} onClose={close} onDeleted={(ids) => onDeleted?.(ids)} />
      )}
      {notice && (
        <div
          className={
            "selectable fixed right-6 bottom-6 z-50 max-w-md rounded-lg px-4 py-2.5 text-sm shadow-lg " +
            (notice.error ? "bg-red-600 text-white" : "bg-slate-900 text-white dark:bg-slate-100 dark:text-slate-900")
          }
          role="status"
        >
          {notice.text}
        </div>
      )}
    </>
  );

  return {
    menu,
    node,
    rename: (s: Session) => setDialog({ kind: "rename", session: s }),
    remove: (sessions: Session[]) => sessions.length && setDialog({ kind: "delete", sessions }),
  };
}

export function RenameDialog({
  title,
  initial,
  hint,
  onClose,
  onSave,
}: {
  title: string;
  initial: string;
  hint?: string;
  onClose: () => void;
  onSave: (name: string) => Promise<unknown>;
}) {
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
            Cancel
          </button>
          <button className="btn btn-primary" onClick={submit} disabled={busy || !name.trim()}>
            Rename
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
      title={one ? "Delete session?" : `Delete ${sessions.length} sessions?`}
      width="max-w-md"
      onClose={onClose}
      footer={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn-danger" onClick={run} disabled={busy}>
            {busy ? "Deleting…" : "Delete"}
          </button>
        </>
      }
    >
      <div className="space-y-3 text-sm text-slate-600 dark:text-slate-300">
        {one ? (
          <p>
            “{sessions[0].name}” and its {cards} analysed card{cards === 1 ? "" : "s"} will be removed from this
            computer.
          </p>
        ) : (
          <>
            <p>
              These sessions and their {cards} analysed card{cards === 1 ? "" : "s"} will be removed from this computer:
            </p>
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
          Thumbnails and grading images go with them.{" "}
          {scanned
            ? "Images scanned by cardloader are deleted too; image folders you picked are never touched."
            : "Your original images are never touched."}
        </p>
        {running && <p className="text-amber-700 dark:text-amber-400">Running sessions are stopped first.</p>}
        {remote > 0 && (
          <div className="rounded-md border border-slate-200 p-3 dark:border-slate-800">
            <Check
              label={one ? "Also delete the Cards Cloud copy" : `Also delete the Cards Cloud copies (${remote})`}
              checked={withRemote}
              onChange={setWithRemote}
            />
            <p className="mt-1 pl-6 text-xs text-slate-500">
              {withRemote
                ? "The remote session, its cards and images are deleted for everyone in the workspace."
                : "Otherwise the copy stays in Cards Cloud and can still be browsed there."}
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
  const [dialog, setDialog] = useState<{ kind: "rename" | "delete"; session: RemoteSession } | null>(null);
  const close = useCallback(() => setDialog(null), []);

  const menu = (s: RemoteSession, { open }: { open?: () => void } = {}): MenuItem[] =>
    [
      open && { label: "Open", icon: FolderOpen, onClick: open },
      { label: "Rename…", icon: Pencil, onClick: () => setDialog({ kind: "rename", session: s }) },
      { label: "Delete…", icon: Trash2, danger: true, onClick: () => setDialog({ kind: "delete", session: s }) },
    ].filter(Boolean) as MenuItem[];

  const node: ReactNode = (
    <>
      {dialog?.kind === "rename" && (
        <RenameDialog
          title="Rename session in Cards Cloud"
          initial={dialog.session.name}
          onClose={close}
          onSave={(name) => api.renameRemoteSession(dialog.session.id, name).then((s) => onRenamed?.(s))}
        />
      )}
      {dialog?.kind === "delete" && (
        <ConfirmDialog
          title="Delete session from Cards Cloud?"
          confirm="Delete"
          onClose={close}
          onConfirm={() => api.deleteRemoteSession(dialog.session.id).then(() => onDeleted?.(dialog.session.id))}
        >
          “{dialog.session.name || "Untitled"}” and its {dialog.session.identification_count} card
          {dialog.session.identification_count === 1 ? "" : "s"} with their images are deleted from Cards Cloud for
          everyone in the workspace. A local session it was uploaded from keeps its cards; uploading it again creates a
          new session there.
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
}: {
  title: string;
  confirm: string;
  children: ReactNode;
  onClose: () => void;
  onConfirm: () => Promise<unknown>;
}) {
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
            Cancel
          </button>
          <button
            className="btn btn-danger"
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
