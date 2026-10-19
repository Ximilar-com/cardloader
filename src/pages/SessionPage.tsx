import { openPath } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import {
  ArrowLeft,
  Cloud,
  CloudUpload,
  FolderOpen,
  Pause,
  Play,
  RotateCcw,
  Square,
  Pencil,
  ArrowDownToLine,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import DetailPanel from "../components/DetailPanel";
import Menu from "../components/Menu";
import { ErrorNote } from "../components/Modal";
import { useSessionActions } from "../components/SessionActions";
import ProgressBar from "../components/ProgressBar";
import SessionTable from "../components/SessionTable";
import StatusChip from "../components/StatusChip";
import {
  api,
  errorText,
  onItemUpdated,
  onSessionUpdated,
  type ItemDetail,
  type ItemSummary,
  type Session,
} from "../lib/api";
import { rowFromItem, viewFromItem } from "../lib/cardview";
import { dateTime, duration, eta, GRADING_LABELS, ID_MODE_LABELS, price, SOURCE_LABELS } from "../lib/format";

export default function SessionPage() {
  const { id = "" } = useParams();
  return <SessionView key={id} id={id} />;
}

function SessionView({ id }: { id: string }) {
  const navigate = useNavigate();
  const [session, setSession] = useState<Session | null>(null);
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<ItemDetail | null>(null);
  const [fresh, setFresh] = useState<Set<string>>(new Set());
  const [follow, setFollow] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const actions = useSessionActions({ onDeleted: () => navigate("/sessions") });
  const [secondsLeft, setSecondsLeft] = useState<number | null>(null);
  const samples = useRef<[number, number][]>([]);
  const selectedRef = useRef<string | null>(null);
  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);

  useEffect(() => {
    api
      .getSession(id)
      .then(setSession)
      .catch((e) => setError(errorText(e)));
    api
      .listItems(id)
      .then(setItems)
      .catch((e) => setError(errorText(e)));

    const unSession = onSessionUpdated((s) => {
      if (s.id !== id) return;
      setSession(s);
      const processed = s.counts.done + s.counts.errors + s.counts.grading;
      const now = Date.now();
      if (s.status === "running") {
        samples.current = [...samples.current.filter(([t]) => now - t < 90_000), [now, processed]];
        setSecondsLeft(eta(samples.current, s.counts.queued + s.counts.inProgress));
      } else {
        samples.current = [];
        setSecondsLeft(null);
      }
    });
    const unItem = onItemUpdated((item) => {
      if (item.sessionId !== id) return;
      setItems((list) => {
        const i = list.findIndex((x) => x.id === item.id);
        if (i < 0) {
          const key = String(item.id);
          setFresh((f) => new Set(f).add(key));
          setTimeout(
            () =>
              setFresh((f) => {
                const n = new Set(f);
                n.delete(key);
                return n;
              }),
            1500,
          );
          return [...list, item].sort((a, b) => a.seq - b.seq);
        }
        const next = [...list];
        next[i] = item;
        return next;
      });
      if (String(item.id) === selectedRef.current) api.getItem(item.id).then(setDetail);
    });
    return () => {
      void unSession.then((f) => f());
      void unItem.then((f) => f());
    };
  }, [id]);

  const select = useCallback((key: string) => {
    setSelected(key);
    setFollow(false);
    api
      .getItem(Number(key))
      .then(setDetail)
      .catch((e) => setError(errorText(e)));
  }, []);

  const rows = useMemo(() => items.map(rowFromItem), [items]);
  // Auto mode reads slab labels it sees even when the session didn't ask for them.
  const showSlab = session?.options.slab === true || session?.idMode === "slab" || rows.some((r) => r.slab);
  const act = (f: () => Promise<unknown>) => {
    setError(null);
    f().catch((e) => setError(errorText(e)));
  };

  if (!session) {
    return <div className="p-6">{error ? <ErrorNote>{error}</ErrorNote> : null}</div>;
  }

  const { counts, status } = session;
  const running = status === "running";
  const active = running || status === "paused";

  return (
    <div className="flex h-full">
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="border-b border-slate-200 px-6 pt-4 pb-3 dark:border-slate-800">
          <div className="flex items-center gap-3">
            <Link to="/sessions" className="btn btn-ghost px-1.5" aria-label="Back to sessions">
              <ArrowLeft className="h-4 w-4" />
            </Link>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <EditableTitle
                  name={session.name}
                  onRename={(name) =>
                    api
                      .renameSession(id, name)
                      .then(setSession)
                      .catch((e) => setError(errorText(e)))
                  }
                />
                <StatusChip status={status} />
              </div>
              <p className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs whitespace-nowrap text-slate-500">
                <span>{SOURCE_LABELS[session.sourceKind]}</span>
                {session.sourceKind !== "scanner" ? (
                  <button
                    className="flex min-w-0 shrink items-center gap-1 hover:underline"
                    onClick={() => openPath(session.sourceRef)}
                    title={session.sourceRef}
                  >
                    <FolderOpen className="h-3 w-3 shrink-0" /> <span className="truncate">{session.sourceRef}</span>
                  </button>
                ) : (
                  <span className="truncate">{session.sourceRef}</span>
                )}
                <span>·</span>
                <span>{session.options.engine === "local" ? "Local model" : ID_MODE_LABELS[session.idMode]}</span>
                {session.options.grading !== "off" && (
                  <>
                    <span>·</span>
                    <span>Grading: {GRADING_LABELS[session.options.grading].split(" (")[0]}</span>
                  </>
                )}
                <span>·</span>
                <span>Created {dateTime(session.createdAt)}</span>
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              {running && (
                <button className="btn" onClick={() => act(() => api.pauseSession(id))}>
                  <Pause className="h-4 w-4" /> Pause
                </button>
              )}
              {status === "paused" && (
                <button className="btn btn-primary" onClick={() => act(() => api.runSession(id))}>
                  <Play className="h-4 w-4" /> Resume
                </button>
              )}
              {active && (
                <button className="btn" onClick={() => act(() => api.stopSession(id))}>
                  <Square className="h-4 w-4" /> Stop
                </button>
              )}
              {!active && (
                <button className="btn btn-primary" onClick={() => act(() => api.runSession(id))}>
                  <Play className="h-4 w-4" />{" "}
                  {status === "draft" ? "Run" : session.sourceKind === "scanner" ? "Scan more" : "Run again"}
                </button>
              )}
              {!active && counts.errors > 0 && (
                <button className="btn" onClick={() => act(() => api.retryItems(id).then(() => api.runSession(id)))}>
                  <RotateCcw className="h-4 w-4" /> Retry {counts.errors} failed
                </button>
              )}
              {!active && counts.done > 0 && (
                <button
                  className="btn"
                  onClick={() => act(() => api.syncSession(id))}
                  title="Upload this session to Ximilar Cards Cloud"
                >
                  <CloudUpload className="h-4 w-4" /> Upload
                </button>
              )}
              <Menu items={actions.menu(session)} label="Session actions" />
            </div>
          </div>

          <div className="mt-3">
            <ProgressBar counts={counts} active={running} />
            <div className="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-500">
              <span>
                <b className="text-slate-800 tabular-nums dark:text-slate-100">{counts.done}</b> / {counts.total}{" "}
                analysed
              </span>
              {counts.inProgress > 0 && <span className="text-accent">{counts.inProgress} in progress</span>}
              {counts.grading > 0 && <span className="text-violet-600">{counts.grading} grading</span>}
              {counts.errors > 0 && <span className="text-red-600">{counts.errors} failed</span>}
              {counts.queued > 0 && <span>{counts.queued} queued</span>}
              {running && secondsLeft != null && <span>~{duration(secondsLeft)} left</span>}
              {counts.totalPrice > 0 && <span>Total value {price(counts.totalPrice)}</span>}
              <span className="flex-1" />
              {(session.remoteSessionId || session.options.syncRemote) && (
                <span
                  className={clsx("flex items-center gap-1", session.syncError && "text-amber-600")}
                  title={session.syncError ?? undefined}
                >
                  <Cloud className="h-3.5 w-3.5" />
                  {session.syncError
                    ? `Cards Cloud: ${session.syncError}`
                    : session.remoteSessionId
                      ? `Cards Cloud: ${counts.synced} synced`
                      : "Cards Cloud: waiting"}
                  {session.remoteSessionId && (
                    <Link to={`/remote/${session.remoteSessionId}`} className="ml-1 text-accent hover:underline">
                      open
                    </Link>
                  )}
                </span>
              )}
              <button
                className={clsx("flex items-center gap-1", follow ? "text-accent" : "hover:text-slate-700")}
                onClick={() => setFollow((f) => !f)}
                title="Keep the newest card in view"
              >
                <ArrowDownToLine className="h-3.5 w-3.5" /> Follow
              </button>
            </div>
            {session.error && (
              <div className="mt-2">
                <ErrorNote>{session.error}</ErrorNote>
              </div>
            )}
            {error && (
              <div className="mt-2">
                <ErrorNote>{error}</ErrorNote>
              </div>
            )}
          </div>
        </header>

        <div className="min-h-0 flex-1">
          {rows.length > 0 ? (
            <SessionTable
              rows={rows}
              selected={selected}
              onSelect={select}
              fresh={fresh}
              follow={follow && running}
              showSlab={showSlab}
            />
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-slate-500">
              {running ? "Waiting for the first card…" : "No cards yet — press Run."}
            </div>
          )}
        </div>
      </div>

      {detail && selected && (
        <DetailPanel
          view={viewFromItem(detail)}
          onClose={() => {
            setSelected(null);
            setDetail(null);
          }}
          onRetry={
            detail.status === "error" && !active
              ? () => act(() => api.retryItems(id, [detail.id]).then(() => api.runSession(id)))
              : undefined
          }
          onPick={
            detail.status === "done" || detail.status === "grading"
              ? (index) =>
                  api
                    .selectMatch(detail.id, index)
                    .then(setDetail)
                    .catch((e) => setError(errorText(e)))
              : undefined
          }
        />
      )}

      {actions.node}
    </div>
  );
}

/** The session name; click to rename in place (Enter saves, Escape cancels). */
function EditableTitle({ name, onRename }: { name: string; onRename: (name: string) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  // Enter closes the input, which then blurs: save once.
  const editing = useRef(false);
  if (draft == null)
    return (
      <button
        className="group flex min-w-0 items-center gap-1.5 text-left"
        onClick={() => {
          editing.current = true;
          setDraft(name);
        }}
        title="Click to rename"
      >
        <h1 className="truncate text-lg font-semibold">{name}</h1>
        <Pencil className="h-3.5 w-3.5 shrink-0 text-slate-400 opacity-0 group-hover:opacity-100" />
      </button>
    );
  const finish = () => {
    if (!editing.current) return;
    editing.current = false;
    const next = draft.trim();
    setDraft(null);
    if (next && next !== name) onRename(next);
  };
  return (
    <input
      className="input max-w-md py-0.5 text-lg font-semibold"
      value={draft}
      autoFocus
      onFocus={(e) => e.target.select()}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={finish}
      onKeyDown={(e) => {
        if (e.key === "Enter") finish();
        if (e.key === "Escape") {
          editing.current = false;
          setDraft(null);
        }
      }}
      aria-label="Session name"
    />
  );
}
