// Every identified card of every local session: search, filter, totals, where cards are kept and how many of
// each there are.

import clsx from "clsx";
import { BadgeQuestionMark, Check, ExternalLink, MapPin, Search, Undo2, X } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import DetailPanel from "../components/DetailPanel";
import { ErrorNote } from "../components/Modal";
import { RenameDialog } from "../components/SessionActions";
import SessionTable from "../components/SessionTable";
import { toast, toastError } from "../components/Toast";
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
import { price } from "../lib/format";

const FILTERS: [CardFilter, string][] = [
  ["all", "All"],
  ["review", "To review"],
  ["graded", "Graded"],
  ["duplicates", "Duplicates"],
];

export default function CardsPage() {
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
    const t = setTimeout(
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
    return () => clearTimeout(t);
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
            <h1 className="flex-1 text-lg font-semibold">Cards</h1>
            {result && (
              <span className="text-sm text-slate-500">
                <b className="text-slate-800 tabular-nums dark:text-slate-100">{result.total.toLocaleString()}</b> card
                {result.total === 1 ? "" : "s"}
                {result.value > 0 && (
                  <>
                    {" · "}
                    <b className="text-slate-800 tabular-nums dark:text-slate-100">{price(result.value)}</b>
                  </>
                )}
              </span>
            )}
          </div>
          <p className="mt-0.5 text-xs text-slate-500">
            Every identified card of your sessions on this computer. Search by name, set, number, location or notes.
          </p>
        </header>

        {checked.size > 0 ? (
          <div className="flex items-center gap-2 border-b border-slate-200 bg-accent/5 px-4 py-2 text-sm dark:border-slate-800 dark:bg-accent/10">
            <span className="font-medium">{checked.size} selected</span>
            <span className="flex-1" />
            {rows.some((r) => !r.reviewed && checked.has(r.key)) && (
              <button
                className="btn px-2 py-1 text-xs"
                onClick={() =>
                  perSession(checkedCards, (s, ids) => api.reviewItems(s, ids))
                    .then((n) => toast(`Accepted ${n} match${n === 1 ? "" : "es"}`))
                    .catch(toastError)
                }
              >
                <Check className="h-3.5 w-3.5" /> Accept
              </button>
            )}
            {rows.some((r) => r.accepted && checked.has(r.key)) && (
              <button
                className="btn px-2 py-1 text-xs"
                onClick={() =>
                  perSession(checkedCards, (s, ids) => api.reviewItems(s, ids, false))
                    .then((n) => toast(`Unaccepted ${n} match${n === 1 ? "" : "es"}`))
                    .catch(toastError)
                }
                title="Take the accepts back"
              >
                <Undo2 className="h-3.5 w-3.5" /> Unaccept
              </button>
            )}
            <button className="btn px-2 py-1 text-xs" onClick={() => setLocating(checkedCards)}>
              <MapPin className="h-3.5 w-3.5" /> Set location…
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
            {FILTERS.map(([key, label]) => (
              <Chip key={key} on={filter === key} onClick={() => setFilter(key)}>
                {key === "review" && <BadgeQuestionMark className="h-3.5 w-3.5" />}
                {label}
              </Chip>
            ))}
            <span className="flex-1" />
            <label className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400" />
              <input
                className="input w-64 py-1 pl-7 text-sm"
                placeholder="Name, set, number, location…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Escape" && setQuery("")}
                aria-label="Search cards"
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
            Showing the newest {result.cards.length.toLocaleString()} — search or filter to find the others.
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
                  "No cards match."
                ) : (
                  <>
                    No identified cards yet.
                    <Link to="/sessions" className="btn">
                      Go to sessions
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
          position={at >= 0 ? `Card ${at + 1} of ${rows.length}` : undefined}
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
              <ExternalLink className="h-3.5 w-3.5" /> In session “{byId.get(selected)?.sessionName}”
            </Link>
          }
        />
      )}

      {locating && (
        <RenameDialog
          title={`Location of ${locating.length} card${locating.length === 1 ? "" : "s"}`}
          initial=""
          hint="Where they are kept, e.g. “Box 4” or “Binder 2, page 7”. It is searchable and goes into the CSV export."
          confirm="Set location"
          onClose={() => setLocating(null)}
          onSave={(location) =>
            perSession(locating, (s, ids) => api.locateItems(s, ids, location)).then((n) => {
              setChecked(new Set());
              toast(`${n} card${n === 1 ? "" : "s"} now at ${location.trim()}`);
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
