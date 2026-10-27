import { openPath } from "@tauri-apps/plugin-opener";
import clsx from "clsx";
import {
  ArrowLeft,
  Bot,
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
  RotateCw,
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
import { toast, toastError, turnUpright } from "../components/Toast";
import {
  api,
  errorText,
  onEbayProgress,
  onItemUpdated,
  onSessionDeleted,
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
import { dateTime, duration, engineLabels, eta, idModeLabels, price, sourceLabels } from "../lib/format";
import { rich, t, useT } from "../i18n";

export default function SessionPage() {
  const { id = "" } = useParams();
  return <SessionView key={id} id={id} />;
}

function SessionView({ id }: { id: string }) {
  const t = useT();
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
    return f === "review" || f === "failed" || f === "corrected" || f === "agent" ? f : "all";
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
        samples.current = [...samples.current.filter(([at]) => now - at < 90_000), [now, processed]];
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
    // Deleted elsewhere (an agent): nothing left to show here.
    const unDeleted = onSessionDeleted((deleted) => {
      if (deleted === id) navigate("/sessions");
    });
    return () => {
      void unSession.then((f) => f());
      void unItem.then((f) => f());
      void unEbay.then((f) => f());
      void unDeleted.then((f) => f());
    };
  }, [id, navigate]);

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
      agent: rows.filter((r) => r.byAgent).length,
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
        if (ids.length > 1) toast(t("session.toast.accepted", { count: n }));
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
          {/* When the detail panel leaves too little room, the buttons go to their own line. */}
          <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
            <Link to="/sessions" className="btn btn-ghost px-1.5" aria-label={t("session.backToSessions")}>
              <ArrowLeft className="h-4 w-4" />
            </Link>
            <div className="min-w-64 flex-1">
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
              <p className="mt-0.5 flex min-w-0 items-center gap-1.5 overflow-hidden text-xs whitespace-nowrap text-slate-500">
                <span>{sourceLabels()[session.sourceKind]}</span>
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
                <span>
                  {session.options.engine === "local" ? engineLabels().local : idModeLabels()[session.idMode]}
                </span>
                {session.options.grading !== "off" && (
                  <>
                    <span>·</span>
                    <span>{t("session.grading", { mode: t(`status.gradingShort.${session.options.grading}`) })}</span>
                  </>
                )}
                <span>·</span>
                <span>{t("session.created", { date: dateTime(session.createdAt) })}</span>
              </p>
            </div>
            <div className="ml-auto flex shrink-0 items-center gap-2">
              {running && (
                <button className="btn" onClick={() => act(() => api.pauseSession(id))}>
                  <Pause className="h-4 w-4" /> {t("session.pause")}
                </button>
              )}
              {status === "paused" && (
                <button className="btn btn-primary" onClick={() => act(() => api.runSession(id))}>
                  <Play className="h-4 w-4" /> {t("session.resume")}
                </button>
              )}
              {active && (
                <button className="btn" onClick={() => act(() => api.stopSession(id))}>
                  <Square className="h-4 w-4" /> {t("session.stop")}
                </button>
              )}
              {!active && (
                <button className="btn btn-primary" onClick={() => act(() => api.runSession(id))}>
                  <Play className="h-4 w-4" />{" "}
                  {status === "draft"
                    ? t("session.run")
                    : session.sourceKind === "scanner"
                      ? t("session.scanMore")
                      : t("session.runAgain")}
                </button>
              )}
              {!active && counts.errors > 0 && (
                <button
                  className="btn"
                  onClick={() => act(() => api.retryItems(id).then(() => api.runSession(id, false)))}
                >
                  <RotateCcw className="h-4 w-4" /> {t("session.retryFailed", { count: counts.errors })}
                </button>
              )}
              {!active && counts.done > 0 && (counts.synced < counts.done || session.syncError) && (
                <button className="btn" onClick={() => act(() => api.syncSession(id))} title={t("session.uploadTitle")}>
                  <CloudUpload className="h-4 w-4" /> {t("session.upload")}
                </button>
              )}
              {!active && counts.ebayDrafts > 0 && (
                <button
                  className="btn"
                  disabled={!!ebayJob}
                  onClick={() => setPublishing({ ids: null, count: counts.ebayDrafts })}
                  title={t("session.publishOnEbayTitle")}
                >
                  <Send className="h-4 w-4" /> {t("session.publishOnEbay")}
                </button>
              )}
              <Menu items={actions.sellMenu(session)} label={t("session.sellMenu")}>
                <Store className="h-4 w-4" /> {t("session.sellExport")}{" "}
                <ChevronDown className="h-3.5 w-3.5 opacity-60" />
              </Menu>
              <Menu items={actions.menu(session, { sell: false })} label={t("session.sessionActions")} />
            </div>
          </div>

          <div className="mt-3">
            <ProgressBar counts={counts} active={running} />
            <div className="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-500">
              <span>
                {rich(t("session.progress.analysed", { done: counts.done, total: counts.total }), {
                  b: (s) => <b className="text-slate-800 tabular-nums dark:text-slate-100">{s}</b>,
                })}
              </span>
              {counts.inProgress > 0 && (
                <span className="text-accent">{t("session.progress.inProgress", { count: counts.inProgress })}</span>
              )}
              {counts.grading > 0 && (
                <span className="text-violet-600">{t("session.progress.grading", { count: counts.grading })}</span>
              )}
              {counts.errors > 0 && (
                <span className="text-red-600">{t("session.progress.failed", { count: counts.errors })}</span>
              )}
              {counts.queued > 0 && <span>{t("session.progress.queued", { count: counts.queued })}</span>}
              {running && secondsLeft != null && (
                <span>{t("session.progress.left", { time: duration(secondsLeft) })}</span>
              )}
              {counts.totalPrice > 0 && (
                <span>{t("session.progress.totalValue", { price: price(counts.totalPrice) })}</span>
              )}
              {counts.calls > 0 && (
                <span title={t("session.progress.apiCallsTitle")}>
                  {t("session.progress.apiCalls", { count: counts.calls })}
                </span>
              )}
              <span className="flex-1" />
              {(session.remoteSessionId || session.options.syncRemote) && (
                <span
                  className={clsx("flex items-center gap-1", session.syncError && "text-amber-600")}
                  title={session.syncError ?? undefined}
                >
                  <Cloud className="h-3.5 w-3.5" />
                  {session.syncError
                    ? t("session.cloud.notSynced")
                    : session.remoteSessionId
                      ? t("session.cloud.synced", { count: counts.synced })
                      : t("session.cloud.waiting")}
                  {session.remoteSessionId && (
                    <Link to={`/remote/${session.remoteSessionId}`} className="ml-1 text-accent hover:underline">
                      {t("session.cloud.open")}
                    </Link>
                  )}
                </span>
              )}
              <EbayStatus session={session} job={ebayJob} />
              <button
                className={clsx("flex items-center gap-1", follow ? "text-accent" : "hover:text-slate-700")}
                onClick={() => setFollow((f) => !f)}
                title={t("session.followTitle")}
              >
                <ArrowDownToLine className="h-3.5 w-3.5" /> {t("session.follow")}
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
                action={t("session.tryAgain")}
                onAction={() => act(() => api.syncSession(id))}
              />
            )}
            {session.ebayError && !ebayJob && (
              <Warning
                text={`eBay: ${session.ebayError}`}
                action={t("session.checkAgain")}
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
              <span className="font-medium">{t("session.bulk.selected", { count: checked.size })}</span>
              <span className="flex-1" />
              {/* Accept and Unaccept share the first place: a card shows the one it can do. */}
              {rows.some((r) => (!r.reviewed || r.byAgent) && checked.has(r.key)) && (
                <button
                  className="btn px-2 py-1 text-xs"
                  onClick={() => accept(checkedIds)}
                  title={t("session.bulk.acceptTitle")}
                >
                  <Check className="h-3.5 w-3.5" /> {t("session.bulk.accept")}
                </button>
              )}
              {rows.some((r) => r.accepted && checked.has(r.key)) && (
                <button
                  className="btn px-2 py-1 text-xs"
                  onClick={() =>
                    api
                      .reviewItems(id, checkedIds, false)
                      .then((n) => toast(t("session.toast.unaccepted", { count: n })))
                      .catch(toastError)
                  }
                  title={t("session.bulk.unacceptTitle")}
                >
                  <Undo2 className="h-3.5 w-3.5" /> {t("session.bulk.unaccept")}
                </button>
              )}
              <button className="btn px-2 py-1 text-xs" onClick={() => setLocating(checkedIds)}>
                <MapPin className="h-3.5 w-3.5" /> {t("session.bulk.setLocation")}
              </button>
              <button
                className="btn px-2 py-1 text-xs"
                onClick={() => turnUpright(checkedIds)}
                title={t("session.bulk.turnUprightTitle")}
              >
                <RotateCw className="h-3.5 w-3.5" /> {t("session.bulk.turnUpright")}
              </button>
              <button
                className="btn px-2 py-1 text-xs"
                disabled={active}
                title={active ? t("session.bulk.waitUntilFinished") : t("session.bulk.analyseAgainTitle")}
                onClick={() =>
                  act(() =>
                    api.retryItems(id, checkedIds).then(() => {
                      setChecked(new Set());
                      return api.runSession(id, false);
                    }),
                  )
                }
              >
                <RotateCcw className="h-3.5 w-3.5" /> {t("session.bulk.analyseAgain")}
              </button>
              <button
                className="btn px-2 py-1 text-xs text-red-600 dark:text-red-400"
                disabled={active}
                title={active ? t("session.bulk.stopToDelete") : undefined}
                onClick={() => setDeleting(checkedIds)}
              >
                <Trash2 className="h-3.5 w-3.5" /> {t("session.bulk.delete")}
              </button>
              <button
                className="btn btn-ghost px-1.5"
                onClick={() => setChecked(new Set())}
                aria-label={t("session.bulk.clear")}
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-1.5 border-b border-slate-200 px-4 py-2 dark:border-slate-800">
              <Chip on={filter === "all"} onClick={() => setFilter("all")}>
                {t("session.filter.all", { count: rows.length })}
              </Chip>
              <Chip on={filter === "review"} onClick={() => setFilter("review")} disabled={!tally.review}>
                <CircleQuestionMark className="h-3.5 w-3.5" /> {t("session.filter.review", { count: tally.review })}
              </Chip>
              <Chip on={filter === "failed"} onClick={() => setFilter("failed")} disabled={!tally.failed}>
                {t("session.filter.failed", { count: tally.failed })}
              </Chip>
              <Chip on={filter === "corrected"} onClick={() => setFilter("corrected")} disabled={!tally.corrected}>
                {t("session.filter.corrected", { count: tally.corrected })}
              </Chip>
              {(tally.agent > 0 || filter === "agent") && (
                <Chip on={filter === "agent"} onClick={() => setFilter("agent")} disabled={!tally.agent}>
                  <Bot className="h-3.5 w-3.5" /> {t("session.filter.agent", { count: tally.agent })}
                </Chip>
              )}
              {filter === "review" && shown.length > 0 && (
                <button
                  className="btn ml-1 px-2 py-0.5 text-xs"
                  onClick={() => accept(shown.map((r) => Number(r.key)))}
                  title={t("session.filter.acceptAllTitle")}
                >
                  <Check className="h-3 w-3" /> {t("session.filter.acceptAll", { count: shown.length })}
                </button>
              )}
              {filter === "agent" && shown.length > 0 && (
                <button
                  className="btn ml-1 px-2 py-0.5 text-xs"
                  onClick={() => accept(shown.map((r) => Number(r.key)))}
                  title={t("session.filter.confirmAllTitle")}
                >
                  <Check className="h-3 w-3" /> {t("session.filter.confirmAll", { count: shown.length })}
                </button>
              )}
              <span className="flex-1" />
              <label className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400" />
                <input
                  className="input w-52 py-1 pl-7 text-sm"
                  placeholder={t("session.filter.find")}
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Escape" && setQuery("")}
                  aria-label={t("session.filter.findLabel")}
                />
              </label>
              <button
                className="btn btn-ghost px-1.5"
                onClick={() => setKeysHelp(true)}
                title={t("session.keys.title")}
                aria-label={t("session.keys.title")}
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
              {filter === "review" && !query ? t("session.empty.nothingToReview") : t("session.empty.noMatch")}
              <button
                className="btn"
                onClick={() => {
                  setFilter("all");
                  setQuery("");
                }}
              >
                {t("session.empty.showAll")}
              </button>
            </div>
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-slate-500">
              {running ? t("session.empty.waiting") : t("session.empty.noCards")}
            </div>
          )}
        </div>
      </div>

      {detail && selected && (
        <DetailPanel
          key={selected}
          view={viewFromItem(detail)}
          position={at >= 0 ? t("session.position", { n: at + 1, total: shown.length }) : undefined}
          onPrev={step(-1)}
          onNext={step(1)}
          onAccept={() => accept([detail.id])}
          onUnaccept={() => api.reviewItems(id, [detail.id], false).catch(toastError)}
          onEdit={(edit) => api.updateItem(detail.id, edit).then(setDetail)}
          onDelete={active ? undefined : () => setDeleting([detail.id])}
          onTurn={() =>
            api
              .turnItem(detail.id)
              .then((d) => {
                setDetail(d);
                toast(t("session.toast.turned"));
              })
              .catch(toastError)
          }
          onClose={() => {
            setSelected(null);
            setDetail(null);
          }}
          onRetry={
            detail.status === "error" && !active
              ? () => act(() => api.retryItems(id, [detail.id]).then(() => api.runSession(id, false)))
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
          title={t("session.publish.title")}
          confirm={t("session.publish.confirm", { count: publishing.count })}
          danger={false}
          onClose={() => setPublishing(null)}
          onConfirm={() => api.ebayPublish(id, publishing.ids ?? undefined)}
        >
          {t("session.publish.text", { count: publishing.count })}
        </ConfirmDialog>
      )}

      {deleting && (
        <ConfirmDialog
          title={t("session.deleteCards.title", { count: deleting.length })}
          confirm={t("common.delete")}
          onClose={() => setDeleting(null)}
          onConfirm={() => api.deleteItems(id, deleting).then(() => deleted(deleting))}
        >
          {t("session.deleteCards.text", { count: deleting.length })}
          {session.remoteSessionId && ` ${t("session.deleteCards.remote")}`}
        </ConfirmDialog>
      )}

      {locating && (
        <RenameDialog
          title={t("session.location.title", { count: locating.length })}
          initial=""
          hint={t("session.location.hint")}
          confirm={t("session.location.confirm")}
          onClose={() => setLocating(null)}
          onSave={(location) =>
            api.locateItems(id, locating, location).then((n) => {
              setChecked(new Set());
              toast(t("session.toast.located", { count: n, location: location.trim() }));
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

function keys(): [string, string][] {
  return [
    ["J  /  ↓", t("session.keys.next")],
    ["K  /  ↑", t("session.keys.previous")],
    ["A", t("session.keys.accept")],
    ["1 – 9", t("session.keys.select")],
    ["E", t("session.keys.edit")],
    ["Delete", t("session.keys.delete")],
    ["Esc", t("session.keys.close")],
    [t("session.keys.shiftClick"), t("session.keys.range")],
  ];
}

function KeysHelp({ onClose }: { onClose: () => void }) {
  const t = useT();
  return (
    <Modal title={t("session.keys.title")} width="max-w-sm" onClose={onClose}>
      <dl className="grid grid-cols-[110px_1fr] gap-y-2 text-sm">
        {keys().map(([k, v]) => (
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
      <p className="mt-4 text-xs text-slate-500">{t("session.keys.tip")}</p>
    </Modal>
  );
}

function jobLabels(): Record<EbayProgress["action"], string> {
  return {
    drafts: t("session.ebay.job.drafts"),
    publish: t("session.ebay.job.publish"),
    refresh: t("session.ebay.job.refresh"),
    remove: t("session.ebay.job.remove"),
    "": t("session.ebay.job.working"),
  };
}

/** The session's cards on eBay, or the job running for them. */
function EbayStatus({ session, job }: { session: Session; job: EbayProgress | null }) {
  const t = useT();
  const { ebayDrafts, ebayListed, ebaySold } = session.counts;
  if (!job && !session.ebayError && ebayDrafts + ebayListed + ebaySold === 0) return null;
  const parts = [
    ebayDrafts > 0 && t("session.ebay.drafts", { count: ebayDrafts }),
    ebayListed > 0 && t("session.ebay.listed", { count: ebayListed }),
    ebaySold > 0 && t("session.ebay.sold", { count: ebaySold }),
  ].filter(Boolean);
  return (
    <span
      className={clsx("flex items-center gap-1", session.ebayError && !job && "text-amber-600")}
      title={session.ebayError ?? undefined}
    >
      <Store className="h-3.5 w-3.5" />
      {job ? (
        <>
          {job.total > 0
            ? t("session.ebay.runningProgress", { job: jobLabels()[job.action], done: job.done, total: job.total })
            : t("session.ebay.running", { job: jobLabels()[job.action] })}
          {job.action !== "refresh" && (
            <button
              className="ml-1 text-slate-400 hover:text-slate-700"
              onClick={() => api.ebayCancel(session.id)}
              title={t("session.ebay.stop")}
              aria-label={t("session.ebay.stopLabel")}
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </>
      ) : (
        <>
          {t("session.ebay.status", {
            status: session.ebayError ?? (parts.join(" · ") || t("session.ebay.nothingListed")),
          })}
          <button
            className="ml-1 text-slate-400 hover:text-slate-700"
            onClick={() => api.ebayRefresh(session.id, true).catch(() => {})}
            title={t("session.ebay.check")}
            aria-label={t("session.ebay.checkLabel")}
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
  const t = useT();
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
        title={t("session.title.rename")}
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
      aria-label={t("session.title.name")}
    />
  );
}
