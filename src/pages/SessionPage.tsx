import { openPath } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import {
  ArrowLeft,
  CircleQuestionMark,
  Check,
  ChevronDown,
  Cloud,
  CloudUpload,
  FolderOpen,
  Keyboard,
  MapPin,
  Pause,
  Play,
  RotateCcw,
  Search,
  Square,
  Pencil,
  ArrowDownToLine,
  RefreshCw,
  Send,
  Store,
  Trash2,
  Undo2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import DetailPanel from "../components/DetailPanel";
import { EbaySection } from "../components/Ebay";
import Menu from "../components/Menu";
import Modal, { ErrorNote } from "../components/Modal";
import { ConfirmDialog, RenameDialog, useSessionActions } from "../components/SessionActions";
import ProgressBar from "../components/ProgressBar";
import SessionTable from "../components/SessionTable";
import StatusChip from "../components/StatusChip";
import { toast, toastError } from "../components/Toast";
import {
  api,
  errorText,
  onEbayProgress,
  onItemUpdated,
  onSessionUpdated,
  type EbayProgress,
  type ItemDetail,
  type ItemSummary,
  type Session,
} from "../lib/api";
import {
  filterRows,
  rowFromItem,
  sortRows,
  viewFromItem,
  type RowFilter,
  type Sort,
  type SortKey,
} from "../lib/cardview";
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
  const [ebayJob, setEbayJob] = useState<EbayProgress | null>(null);
  /** Cards to publish (all drafts: null), awaiting confirmation. */
  const [publishing, setPublishing] = useState<{ ids: number[] | null; count: number } | null>(null);
  const [params] = useSearchParams();
  const [filter, setFilter] = useState<RowFilter>(() => {
    const f = params.get("filter");
    return f === "review" || f === "failed" || f === "corrected" ? f : "all";
  });
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort | null>(null);
  /** Cards ticked for a bulk action, and the last one ticked (Shift extends from it). */
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const anchor = useRef<string | null>(null);
  /** Cards to delete, or to give a location, awaiting the dialog. */
  const [deleting, setDeleting] = useState<number[] | null>(null);
  const [locating, setLocating] = useState<number[] | null>(null);
  const [keysHelp, setKeysHelp] = useState(false);
  const samples = useRef<[number, number][]>([]);
  const selectedRef = useRef<string | null>(null);
  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);

  useEffect(() => {
    api
      .getSession(id)
      .then((s) => {
        setSession(s);
        // Listings may have sold since: read them back (eBay limits how often).
        if (s.counts.ebayListed > 0 || s.counts.ebayDrafts > 0) api.ebayRefresh(id).catch(() => {});
      })
      .catch((e) => setError(errorText(e)));
    api.ebayJob(id).then((job) => setEbayJob(job?.running ? job : null));
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
    const unEbay = onEbayProgress((p) => {
      if (p.sessionId === id) setEbayJob(p.running ? p : null);
    });
    return () => {
      void unSession.then((f) => f());
      void unItem.then((f) => f());
      void unEbay.then((f) => f());
    };
  }, [id]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "?" || (e.target as HTMLElement)?.closest("input, textarea, select")) return;
      setKeysHelp(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const select = useCallback((key: string) => {
    setSelected(key);
    setFollow(false);
    api
      .getItem(Number(key))
      .then(setDetail)
      .catch((e) => setError(errorText(e)));
  }, []);

  const rows = useMemo(() => items.map(rowFromItem), [items]);
  const shown = useMemo(() => sortRows(filterRows(rows, filter, query), sort), [rows, filter, query, sort]);
  const tally = useMemo(
    () => ({
      review: rows.filter((r) => r.toReview).length,
      failed: rows.filter((r) => r.status === "error").length,
      corrected: rows.filter((r) => r.picked || r.edited).length,
    }),
    [rows],
  );
  const checkedIds = [...checked].map(Number);

  const onSort = (key: SortKey) =>
    setSort((s) =>
      s?.key !== key ? { key, desc: key === "price" || key === "grade" } : s.desc ? null : { key, desc: true },
    );

  const onCheck = (key: string, range: boolean) => {
    const next = new Set(checked);
    const from = shown.findIndex((r) => r.key === anchor.current);
    const to = shown.findIndex((r) => r.key === key);
    if (range && from >= 0 && to >= 0) {
      const on = !checked.has(key);
      for (const r of shown.slice(Math.min(from, to), Math.max(from, to) + 1)) {
        if (on) next.add(r.key);
        else next.delete(r.key);
      }
    } else if (next.has(key)) next.delete(key);
    else next.add(key);
    anchor.current = key;
    setChecked(next);
  };
  const allShownChecked = shown.length > 0 && shown.every((r) => checked.has(r.key));
  const onCheckAll = () => setChecked(allShownChecked ? new Set() : new Set(shown.map((r) => r.key)));

  /** Accepts the matches; one card at a time moves on to the next card. */
  const accept = (ids: number[]) => {
    const i = shown.findIndex((r) => r.key === selected);
    const next = ids.length === 1 && String(ids[0]) === selected ? shown[i + 1]?.key : undefined;
    api
      .reviewItems(id, ids)
      .then((n) => {
        if (ids.length > 1) toast(`Accepted ${n} match${n === 1 ? "" : "es"}`);
        if (next) select(next);
      })
      .catch(toastError);
  };

  const deleted = (ids: number[]) => {
    const gone = new Set(ids);
    setItems((list) => list.filter((i) => !gone.has(i.id)));
    setChecked(new Set());
    if (selected && gone.has(Number(selected))) {
      setSelected(null);
      setDetail(null);
    }
  };

  const at = shown.findIndex((r) => r.key === selected);
  const step = (d: number) => {
    const r = shown[at + d];
    return r ? () => select(r.key) : undefined;
  };
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
              {!active && counts.done > 0 && (counts.synced < counts.done || session.syncError) && (
                <button
                  className="btn"
                  onClick={() => act(() => api.syncSession(id))}
                  title="Upload this session to Ximilar Cards Cloud"
                >
                  <CloudUpload className="h-4 w-4" /> Upload
                </button>
              )}
              {!active && counts.ebayDrafts > 0 && (
                <button
                  className="btn"
                  disabled={!!ebayJob}
                  onClick={() => setPublishing({ ids: null, count: counts.ebayDrafts })}
                  title="Publish the drafts: they go live on eBay"
                >
                  <Send className="h-4 w-4" /> Publish on eBay
                </button>
              )}
              <Menu items={actions.sellMenu(session)} label="List or export the cards">
                <Store className="h-4 w-4" /> Sell / Export <ChevronDown className="h-3.5 w-3.5 opacity-60" />
              </Menu>
              <Menu items={actions.menu(session, { sell: false })} label="Session actions" />
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
              {counts.calls > 0 && (
                <span title="Paid Ximilar API calls these cards took">{counts.calls} API calls</span>
              )}
              <span className="flex-1" />
              {(session.remoteSessionId || session.options.syncRemote) && (
                <span
                  className={clsx("flex items-center gap-1", session.syncError && "text-amber-600")}
                  title={session.syncError ?? undefined}
                >
                  <Cloud className="h-3.5 w-3.5" />
                  {session.syncError
                    ? "Cards Cloud: not synced"
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
              <EbayStatus session={session} job={ebayJob} />
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
            {session.syncError && !running && (
              <Warning
                text={`Cards Cloud: ${session.syncError}`}
                action="Try again"
                onAction={() => act(() => api.syncSession(id))}
              />
            )}
            {session.ebayError && !ebayJob && (
              <Warning
                text={`eBay: ${session.ebayError}`}
                action="Check again"
                onAction={() => act(() => api.ebayRefresh(id, true))}
              />
            )}
            {error && (
              <div className="mt-2">
                <ErrorNote>{error}</ErrorNote>
              </div>
            )}
          </div>
        </header>

        {rows.length > 0 &&
          (checked.size > 0 ? (
            <div className="flex items-center gap-2 border-b border-slate-200 bg-accent/5 px-4 py-2 text-sm dark:border-slate-800 dark:bg-accent/10">
              <span className="font-medium">{checked.size} selected</span>
              <span className="flex-1" />
              {/* Accept and Unaccept share the first place: a card shows the one it can do. */}
              {rows.some((r) => !r.reviewed && checked.has(r.key)) && (
                <button className="btn px-2 py-1 text-xs" onClick={() => accept(checkedIds)} title="Accept the matches">
                  <Check className="h-3.5 w-3.5" /> Accept
                </button>
              )}
              {rows.some((r) => r.accepted && checked.has(r.key)) && (
                <button
                  className="btn px-2 py-1 text-xs"
                  onClick={() =>
                    api
                      .reviewItems(id, checkedIds, false)
                      .then((n) => toast(`Unaccepted ${n} match${n === 1 ? "" : "es"}`))
                      .catch(toastError)
                  }
                  title="Take the accepts back"
                >
                  <Undo2 className="h-3.5 w-3.5" /> Unaccept
                </button>
              )}
              <button className="btn px-2 py-1 text-xs" onClick={() => setLocating(checkedIds)}>
                <MapPin className="h-3.5 w-3.5" /> Set location…
              </button>
              <button
                className="btn px-2 py-1 text-xs"
                disabled={active}
                title={active ? "Wait until the session has finished" : "Analyse these cards again (uses API calls)"}
                onClick={() =>
                  act(() =>
                    api.retryItems(id, checkedIds).then(() => {
                      setChecked(new Set());
                      return api.runSession(id);
                    }),
                  )
                }
              >
                <RotateCcw className="h-3.5 w-3.5" /> Analyse again
              </button>
              <button
                className="btn px-2 py-1 text-xs text-red-600 dark:text-red-400"
                disabled={active}
                title={active ? "Stop the session to delete cards" : undefined}
                onClick={() => setDeleting(checkedIds)}
              >
                <Trash2 className="h-3.5 w-3.5" /> Delete…
              </button>
              <button
                className="btn btn-ghost px-1.5"
                onClick={() => setChecked(new Set())}
                aria-label="Clear the selection"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-1.5 border-b border-slate-200 px-4 py-2 dark:border-slate-800">
              <Chip on={filter === "all"} onClick={() => setFilter("all")}>
                All {rows.length}
              </Chip>
              <Chip on={filter === "review"} onClick={() => setFilter("review")} disabled={!tally.review}>
                <CircleQuestionMark className="h-3.5 w-3.5" /> To review {tally.review}
              </Chip>
              <Chip on={filter === "failed"} onClick={() => setFilter("failed")} disabled={!tally.failed}>
                Failed {tally.failed}
              </Chip>
              <Chip on={filter === "corrected"} onClick={() => setFilter("corrected")} disabled={!tally.corrected}>
                Corrected {tally.corrected}
              </Chip>
              {filter === "review" && shown.length > 0 && (
                <button
                  className="btn ml-1 px-2 py-0.5 text-xs"
                  onClick={() => accept(shown.map((r) => Number(r.key)))}
                  title="Accept every match in this list"
                >
                  <Check className="h-3 w-3" /> Accept all {shown.length}
                </button>
              )}
              <span className="flex-1" />
              <label className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400" />
                <input
                  className="input w-52 py-1 pl-7 text-sm"
                  placeholder="Find a card…"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Escape" && setQuery("")}
                  aria-label="Find a card"
                />
              </label>
              <button
                className="btn btn-ghost px-1.5"
                onClick={() => setKeysHelp(true)}
                title="Keyboard shortcuts"
                aria-label="Keyboard shortcuts"
              >
                <Keyboard className="h-4 w-4" />
              </button>
            </div>
          ))}

        <div className="min-h-0 flex-1">
          {shown.length > 0 ? (
            <SessionTable
              rows={shown}
              selected={selected}
              onSelect={select}
              fresh={fresh}
              follow={follow && running && filter === "all" && !sort && !query}
              showSlab={showSlab}
              checked={checked}
              onCheck={onCheck}
              onCheckAll={onCheckAll}
              sort={sort}
              onSort={onSort}
            />
          ) : rows.length > 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-slate-500">
              {filter === "review" && !query ? "Nothing left to review." : "No cards match."}
              <button
                className="btn"
                onClick={() => {
                  setFilter("all");
                  setQuery("");
                }}
              >
                Show all cards
              </button>
            </div>
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-slate-500">
              {running ? "Waiting for the first card…" : "No cards yet — press Run."}
            </div>
          )}
        </div>
      </div>

      {detail && selected && (
        <DetailPanel
          key={selected}
          view={viewFromItem(detail)}
          position={at >= 0 ? `Card ${at + 1} of ${shown.length}` : undefined}
          onPrev={step(-1)}
          onNext={step(1)}
          onAccept={() => accept([detail.id])}
          onUnaccept={() => api.reviewItems(id, [detail.id], false).catch(toastError)}
          onEdit={(edit) => api.updateItem(detail.id, edit).then(setDetail)}
          onDelete={active ? undefined : () => setDeleting([detail.id])}
          onClose={() => {
            setSelected(null);
            setDetail(null);
          }}
          onRetry={
            detail.status === "error" && !active
              ? () => act(() => api.retryItems(id, [detail.id]).then(() => api.runSession(id)))
              : undefined
          }
          ebay={
            detail.ebay && (
              <EbaySection
                listing={detail.ebay}
                busy={!!ebayJob || active}
                onPublish={() => setPublishing({ ids: [detail.id], count: 1 })}
                onRemove={() => act(() => api.ebayRemove(id, [detail.id]))}
              />
            )
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

      {publishing && (
        <ConfirmDialog
          title="Publish on eBay?"
          confirm={`Publish ${publishing.count} listing${publishing.count === 1 ? "" : "s"}`}
          danger={false}
          onClose={() => setPublishing(null)}
          onConfirm={() => api.ebayPublish(id, publishing.ids ?? undefined)}
        >
          {publishing.count === 1 ? "The draft goes" : `${publishing.count} drafts go`} live on eBay at the prices they
          were drafted with. eBay’s fees apply once listed or sold.
        </ConfirmDialog>
      )}

      {deleting && (
        <ConfirmDialog
          title={deleting.length === 1 ? "Delete this card?" : `Delete ${deleting.length} cards?`}
          confirm="Delete"
          onClose={() => setDeleting(null)}
          onConfirm={() => api.deleteItems(id, deleting).then(() => deleted(deleting))}
        >
          {deleting.length === 1 ? "The card is" : "The cards are"} removed from this session with their thumbnails;
          your images stay where they are, and running the session again doesn’t bring{" "}
          {deleting.length === 1 ? "it" : "them"} back.
          {session.remoteSessionId && " The Cards Cloud copy keeps them."}
        </ConfirmDialog>
      )}

      {locating && (
        <RenameDialog
          title={`Location of ${locating.length} card${locating.length === 1 ? "" : "s"}`}
          initial=""
          hint="Where they are kept, e.g. “Box 4” or “Binder 2, page 7”. It is searchable and goes into the CSV export."
          confirm="Set location"
          onClose={() => setLocating(null)}
          onSave={(location) =>
            api.locateItems(id, locating, location).then((n) => {
              setChecked(new Set());
              toast(`${n} card${n === 1 ? "" : "s"} now at ${location.trim()}`);
            })
          }
        />
      )}

      {keysHelp && <KeysHelp onClose={() => setKeysHelp(false)} />}

      {actions.node}
    </div>
  );
}

function Chip({
  on,
  disabled,
  onClick,
  children,
}: {
  on: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      className={clsx(
        "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium tabular-nums disabled:opacity-40",
        on
          ? "bg-accent text-white"
          : "bg-slate-100 text-slate-600 enabled:hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-300 dark:enabled:hover:bg-slate-700",
      )}
      disabled={disabled && !on}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function Warning({ text, action, onAction }: { text: string; action: string; onAction: () => void }) {
  return (
    <div className="selectable mt-2 flex items-center gap-3 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-300">
      <span className="flex-1">{text}</span>
      <button className="btn px-2 py-0.5 text-xs" onClick={onAction}>
        {action}
      </button>
    </div>
  );
}

const KEYS: [string, string][] = [
  ["J  /  ↓", "Next card"],
  ["K  /  ↑", "Previous card"],
  ["A", "Accept the match (again: unaccept)"],
  ["1 – 9", "Select that candidate"],
  ["E", "Edit the card"],
  ["Delete", "Delete the card"],
  ["Esc", "Close the card"],
  ["Shift + click", "Select a range of cards"],
];

function KeysHelp({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="Keyboard shortcuts" width="max-w-sm" onClose={onClose}>
      <dl className="grid grid-cols-[110px_1fr] gap-y-2 text-sm">
        {KEYS.map(([k, v]) => (
          <div key={k} className="contents">
            <dt>
              <kbd className="rounded border border-slate-300 bg-slate-50 px-1.5 py-0.5 font-mono text-xs dark:border-slate-700 dark:bg-slate-900">
                {k}
              </kbd>
            </dt>
            <dd className="text-slate-600 dark:text-slate-300">{v}</dd>
          </div>
        ))}
      </dl>
      <p className="mt-4 text-xs text-slate-500">
        Tip: the “To review” filter lists the cards you haven’t accepted yet. Press A to accept one and go to the next;
        sort by Match to see the unsure ones first.
      </p>
    </Modal>
  );
}

const JOB_LABELS: Record<EbayProgress["action"], string> = {
  drafts: "creating drafts",
  publish: "publishing",
  refresh: "checking listings",
  remove: "removing",
  "": "working",
};

/** The session's cards on eBay, or the job running for them. */
function EbayStatus({ session, job }: { session: Session; job: EbayProgress | null }) {
  const { ebayDrafts, ebayListed, ebaySold } = session.counts;
  if (!job && !session.ebayError && ebayDrafts + ebayListed + ebaySold === 0) return null;
  const parts = [
    ebayDrafts > 0 && `${ebayDrafts} draft${ebayDrafts === 1 ? "" : "s"}`,
    ebayListed > 0 && `${ebayListed} listed`,
    ebaySold > 0 && `${ebaySold} sold`,
  ].filter(Boolean);
  return (
    <span
      className={clsx("flex items-center gap-1", session.ebayError && !job && "text-amber-600")}
      title={session.ebayError ?? undefined}
    >
      <Store className="h-3.5 w-3.5" />
      {job ? (
        <>
          eBay: {JOB_LABELS[job.action]}
          {job.total > 0 && ` ${job.done}/${job.total}`}…
          {job.action !== "refresh" && (
            <button
              className="ml-1 text-slate-400 hover:text-slate-700"
              onClick={() => api.ebayCancel(session.id)}
              title="Stop"
              aria-label="Stop the eBay job"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </>
      ) : (
        <>
          eBay: {session.ebayError ?? (parts.join(" · ") || "nothing listed")}
          <button
            className="ml-1 text-slate-400 hover:text-slate-700"
            onClick={() => api.ebayRefresh(session.id, true).catch(() => {})}
            title="Check the listings on eBay now"
            aria-label="Check the listings on eBay"
          >
            <RefreshCw className="h-3 w-3" />
          </button>
        </>
      )}
    </span>
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
