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
import {
  api,
  errorText,
  onSessionDeleted,
  onSessionUpdated,
  type Session,
  type SettingsView,
  type SourceKind,
} from "../lib/api";
import { locale, useT } from "../i18n";
import { dateTime, engineLabels, gradingLabels, idModeLabels, price, sourceLabels } from "../lib/format";

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

/** Short weekday names for the heatmap's rows (Sunday first), every other one blank as space allows. */
function weekdayRows(): string[] {
  const format = new Intl.DateTimeFormat(locale(), { weekday: "short" });
  // 2024-01-07 was a Sunday.
  return Array.from({ length: 7 }, (_, i) => (i % 2 === 1 ? format.format(new Date(2024, 0, 7 + i)) : ""));
}

export default function DashboardPage() {
  const t = useT();
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
    // Sessions an agent creates or deletes come and go too.
    const un = onSessionUpdated((s) =>
      setSessions(
        (list) => list && (list.some((x) => x.id === s.id) ? list.map((x) => (x.id === s.id ? s : x)) : [s, ...list]),
      ),
    );
    const unDeleted = onSessionDeleted((id) => setSessions((list) => list && list.filter((x) => x.id !== id)));
    return () => {
      void un.then((f) => f());
      void unDeleted.then((f) => f());
    };
  }, []);

  const resume = (id: string) => api.runSession(id).catch((e) => setError(errorText(e)));

  const sources = settings?.defaultSources.length ? settings.defaultSources : (["folder"] as SourceKind[]);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-3 border-b border-slate-200 px-6 py-4 dark:border-slate-800">
        <h1 className="flex-1 text-lg font-semibold">{t("dashboard.title")}</h1>
        {sources.slice(1).map((kind) => {
          const Icon = SOURCE_ICONS[kind];
          return (
            <button key={kind} className="btn" onClick={() => setCreating(kind)}>
              <Icon className="h-4 w-4" /> {sourceLabels()[kind]}
            </button>
          );
        })}
        <button className="btn btn-primary" onClick={() => setCreating(sources[0])}>
          <Plus className="h-4 w-4" /> {t("dashboard.newSession")}
        </button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-5xl space-y-5">
          <ErrorNote>{error}</ErrorNote>
          {sessions && <Totals sessions={sessions} calls={calls} />}
          {settings && sessions && <Attention settings={settings} sessions={sessions} onResume={resume} />}
          <div className="grid grid-cols-[3fr_2fr] gap-5">
            <Card title={t("dashboard.lastWeek")}>{cards && <WeekBars cards={cards} />}</Card>
            <Card
              title={t("dashboard.defaultsTitle")}
              action={
                settings && (
                  <button className="btn btn-ghost px-2 py-1 text-xs" onClick={() => show(settings)}>
                    <Wand2 className="h-3.5 w-3.5" /> {t("dashboard.runSetupAgain")}
                  </button>
                )
              }
            >
              {settings && <Defaults settings={settings} />}
            </Card>
          </div>
          <Card title={t("dashboard.activity")}>{cards && <Heatmap cards={cards} />}</Card>
          <Card
            title={t("dashboard.lastSessions")}
            action={
              <Link
                to="/sessions"
                className="flex items-center gap-0.5 text-xs font-medium text-accent hover:underline"
              >
                {t("dashboard.allSessions")} <ChevronRight className="h-3.5 w-3.5" />
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
  const t = useT();
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
        <span className="text-3xl font-semibold tabular-nums">{total.toLocaleString(locale())}</span>
        <span className="ml-2 text-sm text-slate-500">{t("dashboard.week.cardsAnalysed")}</span>
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
                {t("dashboard.week.dayCards", {
                  count: d.n,
                  date: d.date.toLocaleDateString(locale(), { weekday: "short", month: "short", day: "numeric" }),
                })}
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
            {i === 6 ? t("dashboard.week.today") : d.date.toLocaleDateString(locale(), { weekday: "short" })}
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
  const t = useT();
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
  const dateOf = (d: Date) => d.toLocaleDateString(locale(), { dateStyle: "medium" });

  return (
    <div>
      <p className="mb-3 text-sm text-slate-500">{t("dashboard.heatmap.activeDays", { count: active })}</p>
      <div className="relative overflow-x-auto">
        <div className="flex gap-[3px] pl-8" style={{ height: 16 }}>
          {weeks.map((week, i) => {
            const first = week[0];
            const label = first.getDate() <= 7 ? first.toLocaleDateString(locale(), { month: "short" }) : "";
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
            {weekdayRows().map((d, i) => (
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
                          text: n
                            ? t("dashboard.heatmap.dayCards", { count: n, date: dateOf(d) })
                            : t("dashboard.heatmap.dayNone", { date: dateOf(d) }),
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
          <span className="h-3 w-3 rounded-[3px] bg-slate-200 dark:bg-slate-800" /> {t("dashboard.heatmap.noScanning")}
        </span>
        <span className="flex items-center gap-1.5">
          {t("dashboard.heatmap.fewer")}
          {LEVELS.map((c) => (
            <span key={c} className={clsx("h-3 w-3 rounded-[3px]", c)} />
          ))}
          {t("dashboard.heatmap.moreCards")}
        </span>
      </div>
    </div>
  );
}

/** Everything analysed so far, across the local sessions. */
function Totals({ sessions, calls }: { sessions: Session[]; calls: number }) {
  const t = useT();
  const sum = (f: (s: Session) => number) => sessions.reduce((n, s) => n + f(s), 0);
  const tiles: [string, string, string?][] = [
    [
      t("dashboard.totals.cards"),
      sum((s) => s.counts.done).toLocaleString(locale()),
      t("dashboard.totals.inSessions", { count: sessions.length }),
    ],
    [t("dashboard.totals.totalValue"), price(sum((s) => s.counts.totalPrice)), t("dashboard.totals.totalValueSub")],
    [
      t("dashboard.totals.toReview"),
      sum((s) => s.counts.toReview).toLocaleString(locale()),
      t("dashboard.totals.toReviewSub"),
    ],
    [t("dashboard.totals.apiCalls"), calls.toLocaleString(locale()), t("dashboard.totals.apiCallsSub")],
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
  const t = useT();
  const rows: ReactNode[] = [];
  if (!settings.tokenSet && settings.engine === "ximilar")
    rows.push(
      <AttentionRow key="token" icon={KeyRound} text={t("dashboard.attention.noToken")}>
        <Link to="/settings" className="btn px-2 py-1 text-xs">
          {t("dashboard.attention.openSettings")}
        </Link>
      </AttentionRow>,
    );
  for (const s of sessions) {
    const c = s.counts;
    const notes = [
      s.status === "running" && t("dashboard.attention.analysing", { done: c.done + c.errors, total: c.total }),
      s.status === "paused" && t("dashboard.attention.paused"),
      c.toReview > 0 && t("dashboard.attention.toReview", { count: c.toReview }),
      c.errors > 0 && t("dashboard.attention.failed", { count: c.errors }),
      s.syncError && t("dashboard.attention.syncFailed"),
      s.ebayError && t("dashboard.attention.ebayStopped"),
    ].filter(Boolean);
    if (!notes.length) continue;
    const filter = c.toReview > 0 ? "?filter=review" : c.errors > 0 ? "?filter=failed" : "";
    rows.push(
      <AttentionRow key={s.id} icon={SOURCE_ICONS[s.sourceKind]} title={s.name} text={notes.join(" · ")}>
        {s.status === "paused" && (
          <button className="btn px-2 py-1 text-xs" onClick={() => onResume(s.id)}>
            <Play className="h-3 w-3" /> {t("dashboard.attention.resume")}
          </button>
        )}
        <Link to={`/sessions/${s.id}${filter}`} className="btn px-2 py-1 text-xs">
          {c.toReview > 0 ? t("dashboard.attention.review") : t("dashboard.attention.open")}
        </Link>
      </AttentionRow>,
    );
  }
  if (!rows.length) return null;
  return (
    <Card title={t("dashboard.attention.title")}>
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
  const t = useT();
  const game =
    GAME_LABELS[settings.defaultSubcategory] ?? (settings.defaultSubcategory || t("dashboard.defaults.anyGame"));
  const rows: [string, string][] = [
    [t("dashboard.defaults.identifyAs"), idModeLabels()[settings.defaultIdMode]],
    ...(settings.defaultIdMode === "tcg" ? ([[t("dashboard.defaults.game"), game]] as [string, string][]) : []),
    [t("dashboard.defaults.grading"), gradingLabels()[settings.defaultGrading]],
    [t("dashboard.defaults.prices"), settings.priceStats ? t("common.on") : t("common.off")],
    [
      t("dashboard.defaults.cardsFrom"),
      settings.defaultSources.map((s) => sourceLabels()[s]).join(", ") || sourceLabels().folder,
    ],
    [
      t("dashboard.defaults.identifyWith"),
      settings.engine === "local" ? t("dashboard.defaults.localModel") : engineLabels().ximilar,
    ],
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
        <Settings2 className="h-3.5 w-3.5" /> {t("dashboard.defaults.change")}
      </Link>
    </div>
  );
}

function LastSessions({ sessions, onNew }: { sessions: Session[]; onNew: () => void }) {
  const t = useT();
  if (!sessions.length)
    return (
      <div className="flex items-center justify-between rounded-lg border border-dashed border-slate-300 p-4 text-sm text-slate-500 dark:border-slate-700">
        {t("dashboard.noSessions")}
        <button className="btn btn-primary" onClick={onNew}>
          <Plus className="h-4 w-4" /> {t("dashboard.newSession")}
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
                  {sourceLabels()[s.sourceKind]} · {dateTime(s.createdAt)}
                </span>
              </span>
              <span className="text-sm text-slate-500 tabular-nums">
                {t("dashboard.sessionCards", { done: s.counts.done, count: s.counts.total })}
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
