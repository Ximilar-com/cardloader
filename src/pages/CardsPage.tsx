// Every identified card of every local session: search, filter, totals, where cards are kept and how many of
// each there are.

import clsx from "clsx";
import { Bot, CircleQuestionMark, Check, ExternalLink, MapPin, RotateCw, Search, Undo2, X } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import DetailPanel from "../components/DetailPanel";
import { ErrorNote } from "../components/Modal";
import { RenameDialog } from "../components/SessionActions";
import SessionTable from "../components/SessionTable";
import { toast, toastError, turnUpright } from "../components/Toast";
import {
  api,
  errorText,
  onItemUpdated,
  type CardFilter,
  type CardSearch,
  type ItemDetail,
  type ItemSummary,
} from "../lib/api";
import { rowFromItem, sortRows, viewFromItem, type Sort, type SortKey } from "../lib/cardview";
import { rich, useT } from "../i18n";
import { price } from "../lib/format";

const FILTERS: CardFilter[] = ["all", "review", "graded", "duplicates", "agent"];

export default function CardsPage() {
  const t = useT();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<CardFilter>("all");
  const [result, setResult] = useState<CardSearch | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sort, setSort] = useState<Sort | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<ItemDetail | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [locating, setLocating] = useState<ItemSummary[] | null>(null);

  useEffect(() => {
    const timer = setTimeout(
      () =>
        api
          .searchCards(query, filter)
          .then((r) => {
            setResult(r);
            setError(null);
          })
          .catch((e) => setError(errorText(e))),
      query ? 250 : 0,
    );
    return () => clearTimeout(timer);
  }, [query, filter]);

  // Edits, picks and accepts from here or elsewhere: patch the card, keep its session and copies.
  useEffect(() => {
    const un = onItemUpdated((item) =>
      setResult((r) => r && { ...r, cards: r.cards.map((c) => (c.id === item.id ? { ...c, ...item } : c)) }),
    );
    return () => void un.then((f) => f());
  }, []);

  const cards = useMemo(() => result?.cards ?? [], [result]);
  const rows = useMemo(
    () =>
      sortRows(
        cards.map((c) => ({ ...rowFromItem(c), sessionName: c.sessionName, copies: c.copies })),
        sort,
      ),
    [cards, sort],
  );
  const byId = useMemo(() => new Map(cards.map((c) => [String(c.id), c])), [cards]);

  const select = (key: string) => {
    setSelected(key);
    api
      .getItem(Number(key))
      .then(setDetail)
      .catch((e) => setError(errorText(e)));
  };
  const at = rows.findIndex((r) => r.key === selected);
  const step = (d: number) => {
    const r = rows[at + d];
    return r ? () => select(r.key) : undefined;
  };

  /** Runs `f` once per session for the ticked cards (the commands work per session). */
  const perSession = async (list: ItemSummary[], f: (sessionId: string, ids: number[]) => Promise<number>) => {
    const groups = new Map<string, number[]>();
    for (const c of list) groups.set(c.sessionId, [...(groups.get(c.sessionId) ?? []), c.id]);
    let n = 0;
    for (const [sessionId, ids] of groups) n += await f(sessionId, ids);
    return n;
  };
  const checkedCards = [...checked].map((k) => byId.get(k)).filter(Boolean) as ItemSummary[];

  const onSort = (key: SortKey) =>
    setSort((s) =>
      s?.key !== key ? { key, desc: key === "price" || key === "grade" } : s.desc ? null : { key, desc: true },
    );

  return (
    <div className="flex h-full">
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="border-b border-slate-200 px-6 pt-4 pb-3 dark:border-slate-800">
          <div className="flex items-center gap-3">
            <h1 className="flex-1 text-lg font-semibold">{t("cards.title")}</h1>
            {result && (
              <span className="text-sm text-slate-500">
                {rich(t("cards.total", { count: result.total }), {
                  b: (s) => <b className="text-slate-800 tabular-nums dark:text-slate-100">{s}</b>,
                })}
                {result.value > 0 && (
                  <>
                    {" · "}
                    <b className="text-slate-800 tabular-nums dark:text-slate-100">{price(result.value)}</b>
                  </>
                )}
              </span>
            )}
          </div>
          <p className="mt-0.5 text-xs text-slate-500">{t("cards.intro")}</p>
        </header>

        {checked.size > 0 ? (
          <div className="flex items-center gap-2 border-b border-slate-200 bg-accent/5 px-4 py-2 text-sm dark:border-slate-800 dark:bg-accent/10">
            <span className="font-medium">{t("cards.selected", { count: checked.size })}</span>
            <span className="flex-1" />
            {rows.some((r) => (!r.reviewed || r.byAgent) && checked.has(r.key)) && (
              <button
                className="btn px-2 py-1 text-xs"
                onClick={() =>
                  perSession(checkedCards, (s, ids) => api.reviewItems(s, ids))
                    .then((n) => toast(t("cards.accepted", { count: n })))
                    .catch(toastError)
                }
              >
                <Check className="h-3.5 w-3.5" /> {t("cards.accept")}
              </button>
            )}
            {rows.some((r) => r.accepted && checked.has(r.key)) && (
              <button
                className="btn px-2 py-1 text-xs"
                onClick={() =>
                  perSession(checkedCards, (s, ids) => api.reviewItems(s, ids, false))
                    .then((n) => toast(t("cards.unaccepted", { count: n })))
                    .catch(toastError)
                }
                title={t("cards.unacceptTitle")}
              >
                <Undo2 className="h-3.5 w-3.5" /> {t("cards.unaccept")}
              </button>
            )}
            <button className="btn px-2 py-1 text-xs" onClick={() => setLocating(checkedCards)}>
              <MapPin className="h-3.5 w-3.5" /> {t("cards.setLocation")}
            </button>
            <button
              className="btn px-2 py-1 text-xs"
              onClick={() => turnUpright(checkedCards.map((c) => c.id))}
              title={t("cards.turnUprightTitle")}
            >
              <RotateCw className="h-3.5 w-3.5" /> {t("cards.turnUpright")}
            </button>
            <button
              className="btn btn-ghost px-1.5"
              onClick={() => setChecked(new Set())}
              aria-label={t("cards.clearSelection")}
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 border-b border-slate-200 px-4 py-2 dark:border-slate-800">
            {FILTERS.map((key) => (
              <Chip key={key} on={filter === key} onClick={() => setFilter(key)}>
                {key === "review" && <CircleQuestionMark className="h-3.5 w-3.5" />}
                {key === "agent" && <Bot className="h-3.5 w-3.5" />}
                {t(`cards.filter.${key}`)}
              </Chip>
            ))}
            <span className="flex-1" />
            <label className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400" />
              <input
                className="input w-64 py-1 pl-7 text-sm"
                placeholder={t("cards.searchPlaceholder")}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Escape" && setQuery("")}
                aria-label={t("cards.search")}
                autoFocus
              />
            </label>
          </div>
        )}

        {error && (
          <div className="px-4 pt-3">
            <ErrorNote>{error}</ErrorNote>
          </div>
        )}
        {result && result.total > result.cards.length && (
          <p className="border-b border-slate-200 bg-slate-50 px-4 py-1.5 text-xs text-slate-500 dark:border-slate-800 dark:bg-slate-900">
            {t("cards.showingNewest", { count: result.cards.length })}
          </p>
        )}

        <div className="min-h-0 flex-1">
          {rows.length > 0 ? (
            <SessionTable
              rows={rows}
              selected={selected}
              onSelect={select}
              showStatus={false}
              checked={checked}
              onCheck={(key) =>
                setChecked((c) => {
                  const next = new Set(c);
                  if (next.has(key)) next.delete(key);
                  else next.add(key);
                  return next;
                })
              }
              onCheckAll={() =>
                setChecked((c) => (rows.every((r) => c.has(r.key)) ? new Set() : new Set(rows.map((r) => r.key))))
              }
              sort={sort}
              onSort={onSort}
            />
          ) : (
            result && (
              <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-slate-500">
                {query || filter !== "all" ? (
                  t("cards.noMatch")
                ) : (
                  <>
                    {t("cards.empty")}
                    <Link to="/sessions" className="btn">
                      {t("cards.goToSessions")}
                    </Link>
                  </>
                )}
              </div>
            )
          )}
        </div>
      </div>

      {detail && selected && (
        <DetailPanel
          key={selected}
          view={viewFromItem(detail)}
          position={at >= 0 ? t("cards.position", { at: at + 1, count: rows.length }) : undefined}
          onPrev={step(-1)}
          onNext={step(1)}
          onClose={() => {
            setSelected(null);
            setDetail(null);
          }}
          onAccept={() =>
            api
              .reviewItems(detail.sessionId, [detail.id])
              .then(() => step(1)?.())
              .catch(toastError)
          }
          onUnaccept={() => api.reviewItems(detail.sessionId, [detail.id], false).catch(toastError)}
          onEdit={(edit) => api.updateItem(detail.id, edit).then(setDetail)}
          onTurn={() =>
            api
              .turnItem(detail.id)
              .then((d) => {
                setDetail(d);
                toast(t("cards.turned"));
              })
              .catch(toastError)
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
          extra={
            <Link
              to={`/sessions/${detail.sessionId}`}
              className="flex items-center gap-1 text-xs font-medium text-accent hover:underline"
            >
              <ExternalLink className="h-3.5 w-3.5" />{" "}
              {t("cards.inSession", { name: byId.get(selected)?.sessionName ?? "" })}
            </Link>
          }
        />
      )}

      {locating && (
        <RenameDialog
          title={t("cards.location.title", { count: locating.length })}
          initial=""
          hint={t("cards.location.hint")}
          confirm={t("cards.location.confirm")}
          onClose={() => setLocating(null)}
          onSave={(location) =>
            perSession(locating, (s, ids) => api.locateItems(s, ids, location)).then((n) => {
              setChecked(new Set());
              toast(t("cards.location.done", { count: n, location: location.trim() }));
            })
          }
        />
      )}
    </div>
  );
}

function Chip({ on, onClick, children }: { on: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      className={clsx(
        "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium",
        on
          ? "bg-accent text-white"
          : "bg-slate-100 text-slate-600 hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700",
      )}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
