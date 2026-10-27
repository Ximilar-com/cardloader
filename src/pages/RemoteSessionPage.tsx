import { ArrowLeft, Cloud } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import DetailPanel from "../components/DetailPanel";
import Menu from "../components/Menu";
import { ErrorNote } from "../components/Modal";
import { useRemoteSessionActions } from "../components/SessionActions";
import SessionTable from "../components/SessionTable";
import StatusChip from "../components/StatusChip";
import { api, errorText, type Json, type RemoteItem, type RemoteSession } from "../lib/api";
import { rowFromRemote, viewFromRemote } from "../lib/cardview";
import { useT } from "../i18n";
import { dateTime, grade, price } from "../lib/format";

export default function RemoteSessionPage() {
  const t = useT();
  const { id = "" } = useParams();
  const [session, setSession] = useState<RemoteSession | null>(null);
  const [summary, setSummary] = useState<Json>(null);
  const [items, setItems] = useState<RemoteItem[]>([]);
  const [count, setCount] = useState(0);
  const [nextPage, setNextPage] = useState<number | null>(1);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();
  const actions = useRemoteSessionActions({
    onRenamed: (r) => setSession((s) => s && { ...s, name: r.name }),
    onDeleted: () => navigate("/sessions?tab=remote"),
  });

  const fetchPage = useCallback(
    (page: number) =>
      api
        .listRemoteItems(id, page)
        .then((p) => {
          setItems((list) => (page === 1 ? p.results : [...list, ...p.results]));
          setCount(p.count);
          setNextPage(p.next ? page + 1 : null);
        })
        .catch((e) => setError(errorText(e)))
        .finally(() => setLoading(false)),
    [id],
  );
  const loadMore = (page: number) => {
    setLoading(true);
    void fetchPage(page);
  };

  useEffect(() => {
    api
      .getRemoteSession(id)
      .then(({ session, summary }) => {
        setSession(session);
        setSummary(summary);
      })
      .catch((e) => setError(errorText(e)));
    void fetchPage(1);
  }, [id, fetchPage]);

  const rows = useMemo(() => items.map((r, i) => rowFromRemote(r, i + 1)), [items]);
  const selectedItem = items.find((r) => r.id === selected);

  return (
    <div className="flex h-full">
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="border-b border-slate-200 px-6 py-4 dark:border-slate-800">
          <div className="flex items-center gap-3">
            <Link to="/sessions?tab=remote" className="btn btn-ghost px-1.5" aria-label={t("common.back")}>
              <ArrowLeft className="h-4 w-4" />
            </Link>
            <Cloud className="h-5 w-5 text-accent" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <h1 className="truncate text-lg font-semibold">
                  {session?.name || t("sessions.remoteSession.untitled")}
                </h1>
                {session && <StatusChip status={session.status} />}
              </div>
              {session && (
                <p className="mt-0.5 text-xs text-slate-500">
                  {session.ended
                    ? t("sessions.remoteSession.startedEnded", {
                        client: session.client?.name ?? session.platform,
                        date: dateTime(session.started),
                        ended: dateTime(session.ended),
                      })
                    : t("sessions.remoteSession.started", {
                        client: session.client?.name ?? session.platform,
                        date: dateTime(session.started),
                      })}
                </p>
              )}
            </div>
            {session && <Menu items={actions.menu(session)} label={t("sessions.remoteSession.actions")} />}
          </div>
          {summary && (
            <div className="mt-3 grid grid-cols-2 gap-2 sm:grid-cols-5">
              <Stat label={t("sessions.remoteSession.cards")} value={String(summary.identifications ?? count)} />
              <Stat label={t("sessions.remoteSession.distinct")} value={String(summary.distinct_cards ?? "—")} />
              <Stat label={t("sessions.remoteSession.totalValue")} value={price(summary.total_price)} />
              <Stat label={t("sessions.remoteSession.paidCalls")} value={String(summary.paid_calls ?? "—")} />
              <Stat
                label={t("sessions.remoteSession.avgGrade")}
                value={summary.average_grade != null ? grade(summary.average_grade) : "—"}
              />
            </div>
          )}
          {error && (
            <div className="mt-3">
              <ErrorNote>{error}</ErrorNote>
            </div>
          )}
        </header>
        <div className="min-h-0 flex-1">
          <SessionTable rows={rows} selected={selected} onSelect={setSelected} showStatus={false} />
        </div>
        <div className="flex items-center justify-between border-t border-slate-200 px-6 py-2 text-xs text-slate-500 dark:border-slate-800">
          <span>{t("sessions.remoteSession.loadedOf", { shown: items.length, count })}</span>
          {nextPage && (
            <button className="btn" disabled={loading} onClick={() => loadMore(nextPage)}>
              {loading ? t("common.loading") : t("sessions.remoteSession.loadMore")}
            </button>
          )}
        </div>
      </div>
      {actions.node}
      {selectedItem && <DetailPanel view={viewFromRemote(selectedItem)} onClose={() => setSelected(null)} />}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-slate-50 px-3 py-2 dark:bg-slate-900">
      <div className="text-[11px] font-medium tracking-wide text-slate-500 uppercase">{label}</div>
      <div className="text-lg font-semibold tabular-nums">{value}</div>
    </div>
  );
}
