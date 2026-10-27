import clsx from "clsx";
import {
  AlertTriangle,
  ChevronRight,
  Eye,
  FileSpreadsheet,
  FolderOpen,
  KeyRound,
  Play,
  Plus,
  ScanLine,
  Settings2,
  Wand2,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { ErrorNote } from "../components/Modal";
import NewSessionDialog, { GAME_LABELS } from "../components/NewSessionDialog";
import { useSetupWizard } from "../components/SetupWizard";
import StatusChip from "../components/StatusChip";
import { api, errorText, onSessionUpdated, type Session, type SettingsView, type SourceKind } from "../lib/api";
import { GRADING_LABELS, ID_MODE_LABELS, SOURCE_LABELS, dateTime, price } from "../lib/format";

const SOURCE_ICONS = { folder: FolderOpen, watch_folder: Eye, scanner: ScanLine, csv: FileSpreadsheet };
/** 53 weeks: the heatmap's columns. */
const HEATMAP_DAYS = 53 * 7;

/** A local date as the backend's day key, `YYYY-MM-DD`. */
function dayKey(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function daysBack(n: number, from = new Date()): Date {
  return new Date(from.getFullYear(), from.getMonth(), from.getDate() - n);
}

const plural = (n: number, word: string) => `${n.toLocaleString()} ${word}${n === 1 ? "" : "s"}`;

export default function DashboardPage() {
  const show = useSetupWizard((s) => s.show);
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [cards, setCards] = useState<Map<string, number> | null>(null);
  const [calls, setCalls] = useState(0);
  const [sessions, setSessions] = useState<Session[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState<SourceKind | null>(null);

  useEffect(() => {
    const fail = (e: unknown) => setError(errorText(e));
    api.getSettings().then(setSettings).catch(fail);
    api
      .dashboardActivity(HEATMAP_DAYS)
      .then((days) => {
        setCards(new Map(days.map((d) => [d.date, d.cards])));
        const week = dayKey(daysBack(6));
        setCalls(days.filter((d) => d.date >= week).reduce((n, d) => n + d.calls, 0));
      })
      .catch(fail);
    api.listSessions().then(setSessions).catch(fail);
    const un = onSessionUpdated((s) => setSessions((list) => list && list.map((x) => (x.id === s.id ? s : x))));
    return () => void un.then((f) => f());
  }, []);

  const resume = (id: string) => api.runSession(id).catch((e) => setError(errorText(e)));

  const sources = settings?.defaultSources.length ? settings.defaultSources : (["folder"] as SourceKind[]);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-3 border-b border-slate-200 px-6 py-4 dark:border-slate-800">
        <h1 className="flex-1 text-lg font-semibold">Home</h1>
        {sources.slice(1).map((kind) => {
          const Icon = SOURCE_ICONS[kind];
          return (
            <button key={kind} className="btn" onClick={() => setCreating(kind)}>
              <Icon className="h-4 w-4" /> {SOURCE_LABELS[kind]}
            </button>
          );
        })}
        <button className="btn btn-primary" onClick={() => setCreating(sources[0])}>
          <Plus className="h-4 w-4" /> New session
        </button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-5xl space-y-5">
          <ErrorNote>{error}</ErrorNote>
          {sessions && <Totals sessions={sessions} calls={calls} />}
          {settings && sessions && <Attention settings={settings} sessions={sessions} onResume={resume} />}
          <div className="grid grid-cols-[3fr_2fr] gap-5">
            <Card title="Last 7 days">{cards && <WeekBars cards={cards} />}</Card>
            <Card
              title="Defaults for new sessions"
              action={
                settings && (
                  <button className="btn btn-ghost px-2 py-1 text-xs" onClick={() => show(settings)}>
                    <Wand2 className="h-3.5 w-3.5" /> Run setup again
                  </button>
                )
              }
            >
              {settings && <Defaults settings={settings} />}
            </Card>
          </div>
          <Card title="Activity">{cards && <Heatmap cards={cards} />}</Card>
          <Card
            title="Last sessions"
            action={
              <Link
                to="/sessions"
                className="flex items-center gap-0.5 text-xs font-medium text-accent hover:underline"
              >
                All sessions <ChevronRight className="h-3.5 w-3.5" />
              </Link>
            }
          >
            {sessions && <LastSessions sessions={sessions.slice(0, 3)} onNew={() => setCreating(sources[0])} />}
          </Card>
        </div>
      </div>
      {creating && <NewSessionDialog initialKind={creating} onClose={() => setCreating(null)} />}
    </div>
  );
}

function Card({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="card p-5">
      <div className="mb-4 flex min-h-7 items-center justify-between gap-3">
        <h2 className="text-sm font-semibold">{title}</h2>
        {action}
      </div>
      {children}
    </section>
  );
}

/** Cards analysed on each of the last 7 days, today last. */
function WeekBars({ cards }: { cards: Map<string, number> }) {
  const [hover, setHover] = useState<number | null>(null);
  const days = Array.from({ length: 7 }, (_, i) => {
    const date = daysBack(6 - i);
    return { date, n: cards.get(dayKey(date)) ?? 0 };
  });
  const total = days.reduce((sum, d) => sum + d.n, 0);
  const max = Math.max(1, ...days.map((d) => d.n));
  const PLOT = 140;

  return (
    <div>
      <div className="mb-4">
        <span className="text-3xl font-semibold tabular-nums">{total.toLocaleString()}</span>
        <span className="ml-2 text-sm text-slate-500">cards analysed</span>
      </div>
      <div className="relative flex gap-2 border-b border-slate-200 dark:border-slate-700" style={{ height: PLOT }}>
        {days.map((d, i) => (
          <div
            key={i}
            className="relative flex flex-1 items-end justify-center"
            onMouseEnter={() => setHover(i)}
            onMouseLeave={() => setHover(null)}
          >
            {hover === i && (
              <div
                className="pointer-events-none absolute z-10 rounded-md bg-slate-900 px-2 py-1 text-xs whitespace-nowrap text-white shadow-lg dark:bg-slate-100 dark:text-slate-900"
                style={{ bottom: Math.max(4, (d.n / max) * (PLOT - 8)) + 6 }}
              >
                {d.date.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })}:{" "}
                {plural(d.n, "card")}
              </div>
            )}
            {d.n > 0 && (
              <div
                className={clsx("w-full max-w-10 rounded-t", hover === i ? "bg-accent-hover" : "bg-accent")}
                style={{ height: Math.max(4, (d.n / max) * (PLOT - 8)) }}
              />
            )}
          </div>
        ))}
      </div>
      <div className="mt-1.5 flex gap-2">
        {days.map((d, i) => (
          <div key={i} className={clsx("flex-1 text-center text-xs", i === 6 ? "font-medium" : "text-slate-500")}>
            {i === 6 ? "Today" : d.date.toLocaleDateString(undefined, { weekday: "short" })}
          </div>
        ))}
      </div>
    </div>
  );
}

/** Heatmap shades, from a few cards to the busiest day's. */
const LEVELS = ["bg-accent/30", "bg-accent/55", "bg-accent/80", "bg-accent"];

/** One square per day for the last 53 weeks, darker the more cards were analysed that day. */
function Heatmap({ cards }: { cards: Map<string, number> }) {
  const [hover, setHover] = useState<{ text: string; x: number; y: number } | null>(null);
  const today = new Date();
  // Columns are weeks starting on Sunday; the last one is the current week, up to today.
  const start = daysBack(today.getDay() + 52 * 7, today);
  const weeks: Date[][] = [];
  for (let d = start; d <= today; d = daysBack(-1, d)) {
    if (d.getDay() === 0) weeks.push([]);
    weeks[weeks.length - 1].push(d);
  }
  const active = [...cards.values()].filter((n) => n > 0).length;
  const busiest = Math.max(1, ...cards.values());
  const CELL = 14;
  const GAP = 3;

  return (
    <div>
      <p className="mb-3 text-sm text-slate-500">Cards were analysed on {plural(active, "day")} in the last year.</p>
      <div className="relative overflow-x-auto">
        <div className="flex gap-[3px] pl-8" style={{ height: 16 }}>
          {weeks.map((week, i) => {
            const first = week[0];
            const label = first.getDate() <= 7 ? first.toLocaleDateString(undefined, { month: "short" }) : "";
            return (
              <div key={i} className="relative shrink-0 text-[10px] text-slate-500" style={{ width: CELL }}>
                <span className="absolute left-0 whitespace-nowrap">{label}</span>
              </div>
            );
          })}
        </div>
        <div className="flex">
          <div
            className="grid w-8 shrink-0 text-[10px] text-slate-500"
            style={{ gridTemplateRows: `repeat(7, ${CELL}px)`, rowGap: GAP }}
          >
            {["", "Mon", "", "Wed", "", "Fri", ""].map((d, i) => (
              <span key={i} className="leading-3">
                {d}
              </span>
            ))}
          </div>
          <div className="flex gap-[3px]" onMouseLeave={() => setHover(null)}>
            {weeks.map((week, i) => (
              <div key={i} className="grid" style={{ gridTemplateRows: `repeat(7, ${CELL}px)`, rowGap: GAP }}>
                {week.map((d) => {
                  const n = cards.get(dayKey(d)) ?? 0;
                  return (
                    <div
                      key={dayKey(d)}
                      className={clsx(
                        "rounded-[3px]",
                        n > 0 ? LEVELS[Math.min(3, Math.floor((n / busiest) * 4))] : "bg-slate-200 dark:bg-slate-800",
                        "hover:ring-2 hover:ring-slate-400/60",
                      )}
                      style={{ width: CELL, height: CELL, gridRow: d.getDay() + 1 }}
                      onMouseEnter={(e) => {
                        const box = e.currentTarget.getBoundingClientRect();
                        const parent = e.currentTarget.closest(".relative")!.getBoundingClientRect();
                        setHover({
                          text: `${d.toLocaleDateString(undefined, { dateStyle: "medium" })}: ${n ? plural(n, "card") : "no scanning"}`,
                          x: box.left - parent.left + CELL / 2,
                          y: box.top - parent.top,
                        });
                      }}
                    />
                  );
                })}
              </div>
            ))}
          </div>
        </div>
        {hover && (
          <div
            className="pointer-events-none absolute z-10 -translate-x-1/2 -translate-y-full rounded-md bg-slate-900 px-2 py-1 text-xs whitespace-nowrap text-white shadow-lg dark:bg-slate-100 dark:text-slate-900"
            style={{ left: hover.x, top: hover.y - 6 }}
          >
            {hover.text}
          </div>
        )}
      </div>
      <div className="mt-3 flex items-center justify-end gap-3 text-xs text-slate-500">
        <span className="flex items-center gap-1.5">
          <span className="h-3 w-3 rounded-[3px] bg-slate-200 dark:bg-slate-800" /> No scanning
        </span>
        <span className="flex items-center gap-1.5">
          Fewer
          {LEVELS.map((c) => (
            <span key={c} className={clsx("h-3 w-3 rounded-[3px]", c)} />
          ))}
          More cards
        </span>
      </div>
    </div>
  );
}

/** Everything analysed so far, across the local sessions. */
function Totals({ sessions, calls }: { sessions: Session[]; calls: number }) {
  const sum = (f: (s: Session) => number) => sessions.reduce((n, s) => n + f(s), 0);
  const tiles: [string, string, string?][] = [
    ["Cards", sum((s) => s.counts.done).toLocaleString(), `in ${plural(sessions.length, "session")}`],
    ["Total value", price(sum((s) => s.counts.totalPrice)), "market prices when analysed"],
    ["To review", sum((s) => s.counts.toReview).toLocaleString(), "unsure or missing matches"],
    ["API calls", calls.toLocaleString(), "last 7 days"],
  ];
  return (
    <div className="grid grid-cols-4 gap-5">
      {tiles.map(([label, value, sub]) => (
        <div key={label} className="card px-5 py-4">
          <div className="text-xs font-medium text-slate-500">{label}</div>
          <div className="mt-1 text-2xl font-semibold tabular-nums">{value}</div>
          {sub && <div className="text-xs text-slate-500">{sub}</div>}
        </div>
      ))}
    </div>
  );
}

/** What is waiting for the user: running and interrupted sessions, cards to review, failures, a missing token. */
function Attention({
  settings,
  sessions,
  onResume,
}: {
  settings: SettingsView;
  sessions: Session[];
  onResume: (id: string) => void;
}) {
  const rows: ReactNode[] = [];
  if (!settings.tokenSet && settings.engine === "ximilar")
    rows.push(
      <AttentionRow key="token" icon={KeyRound} text="Add your Ximilar API token to analyse cards.">
        <Link to="/settings" className="btn px-2 py-1 text-xs">
          Open Settings
        </Link>
      </AttentionRow>,
    );
  for (const s of sessions) {
    const c = s.counts;
    const notes = [
      s.status === "running" && `analysing, ${c.done + c.errors} of ${c.total}`,
      s.status === "paused" && "paused",
      c.toReview > 0 && `${plural(c.toReview, "card")} to review`,
      c.errors > 0 && `${c.errors} failed`,
      s.syncError && "Cards Cloud upload failed",
      s.ebayError && "eBay job stopped",
    ].filter(Boolean);
    if (!notes.length) continue;
    const filter = c.toReview > 0 ? "?filter=review" : c.errors > 0 ? "?filter=failed" : "";
    rows.push(
      <AttentionRow key={s.id} icon={SOURCE_ICONS[s.sourceKind]} title={s.name} text={notes.join(" · ")}>
        {s.status === "paused" && (
          <button className="btn px-2 py-1 text-xs" onClick={() => onResume(s.id)}>
            <Play className="h-3 w-3" /> Resume
          </button>
        )}
        <Link to={`/sessions/${s.id}${filter}`} className="btn px-2 py-1 text-xs">
          {c.toReview > 0 ? "Review" : "Open"}
        </Link>
      </AttentionRow>,
    );
  }
  if (!rows.length) return null;
  return (
    <Card title="Needs attention">
      <ul className="-mx-2 max-h-72 overflow-y-auto">{rows.slice(0, 8)}</ul>
    </Card>
  );
}

function AttentionRow({
  icon: Icon,
  title,
  text,
  children,
}: {
  icon: typeof AlertTriangle;
  title?: string;
  text: string;
  children: ReactNode;
}) {
  return (
    <li className="flex items-center gap-3 rounded-md px-2 py-2">
      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-amber-50 text-amber-600 dark:bg-amber-950/40">
        <Icon className="h-4 w-4" />
      </span>
      <span className="min-w-0 flex-1">
        {title && <span className="block truncate text-sm font-medium">{title}</span>}
        <span className="block text-xs text-slate-500">{text}</span>
      </span>
      <span className="flex shrink-0 gap-2">{children}</span>
    </li>
  );
}

function Defaults({ settings }: { settings: SettingsView }) {
  const rows: [string, string][] = [
    ["Identify as", ID_MODE_LABELS[settings.defaultIdMode]],
    ...(settings.defaultIdMode === "tcg"
      ? ([["Game", GAME_LABELS[settings.defaultSubcategory] ?? (settings.defaultSubcategory || "Any game")]] as [
          string,
          string,
        ][])
      : []),
    ["Grading", GRADING_LABELS[settings.defaultGrading]],
    ["Prices", settings.priceStats ? "On" : "Off"],
    ["Cards from", settings.defaultSources.map((s) => SOURCE_LABELS[s]).join(", ") || SOURCE_LABELS.folder],
    ["Identify with", settings.engine === "local" ? "Local AI model" : "Ximilar API"],
  ];
  return (
    <div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
        {rows.map(([label, value]) => (
          <div key={label} className="contents">
            <dt className="text-slate-500">{label}</dt>
            <dd className="font-medium">{value}</dd>
          </div>
        ))}
      </dl>
      <Link
        to="/settings"
        className="mt-4 inline-flex items-center gap-1 text-xs font-medium text-accent hover:underline"
      >
        <Settings2 className="h-3.5 w-3.5" /> Change in Settings
      </Link>
    </div>
  );
}

function LastSessions({ sessions, onNew }: { sessions: Session[]; onNew: () => void }) {
  if (!sessions.length)
    return (
      <div className="flex items-center justify-between rounded-lg border border-dashed border-slate-300 p-4 text-sm text-slate-500 dark:border-slate-700">
        No sessions yet.
        <button className="btn btn-primary" onClick={onNew}>
          <Plus className="h-4 w-4" /> New session
        </button>
      </div>
    );
  return (
    <ul className="-mx-2">
      {sessions.map((s) => {
        const Icon = SOURCE_ICONS[s.sourceKind];
        return (
          <li key={s.id}>
            <Link
              to={`/sessions/${s.id}`}
              className="flex items-center gap-3 rounded-md px-2 py-2.5 hover:bg-slate-50 dark:hover:bg-slate-800/60"
            >
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-slate-100 text-slate-500 dark:bg-slate-800">
                <Icon className="h-4 w-4" />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium">{s.name}</span>
                <span className="block text-xs text-slate-500">
                  {SOURCE_LABELS[s.sourceKind]} · {dateTime(s.createdAt)}
                </span>
              </span>
              <span className="text-sm text-slate-500 tabular-nums">
                {s.counts.done.toLocaleString()} / {plural(s.counts.total, "card")}
              </span>
              <span className="w-20 text-right text-sm font-medium tabular-nums">{price(s.counts.totalPrice)}</span>
              <StatusChip status={s.status} />
            </Link>
          </li>
        );
      })}
    </ul>
  );
}
