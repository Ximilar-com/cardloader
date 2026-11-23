import { convertFileSrc } from "@tauri-apps/api/core";
import { locale, t } from "../i18n";
import type { Engine, GradingMode, IdMode, Money, Prices, SessionOptions, SourceKind, Stage, Task } from "./api";

/** A price in its currency (US dollars unless said: Ximilar's are), written the way the app's language writes
 * amounts. */
export function price(value: number | null | undefined, currency = "USD"): string {
  return value == null ? "—" : new Intl.NumberFormat(locale(), { style: "currency", currency }).format(value);
}

/** A price without cents from 100 up: short enough for a range in a table cell. */
export function shortPrice(value: number, currency = "USD"): string {
  return new Intl.NumberFormat(locale(), {
    style: "currency",
    currency,
    maximumFractionDigits: value >= 100 ? 0 : 2,
  }).format(value);
}

/** Totals by currency, the largest first ("$1,234.00 · €56.00"); "—" when there are none. */
export function amounts(values: Money[] | null | undefined): string {
  const shown = (values ?? []).filter((m) => m.total > 0);
  return shown.length ? shown.map((m) => price(m.total, m.currency)).join(" · ") : "—";
}

/** Totals by currency added up (sessions' values on the Dashboard), the largest first. */
export function addMoney(lists: Money[][]): Money[] {
  const totals = new Map<string, number>();
  for (const m of lists.flat()) totals.set(m.currency, (totals.get(m.currency) ?? 0) + m.total);
  return [...totals].map(([currency, total]) => ({ currency, total })).sort((a, b) => b.total - a.total);
}

export function grade(value: number | null | undefined): string {
  if (value == null) return "—";
  return value.toLocaleString(locale(), { minimumFractionDigits: 0, maximumFractionDigits: 1 });
}

/** A number shown with `digits` decimals, as the app's language writes them ("0.120", "0,120"). */
export function decimal(value: number, digits: number): string {
  return value.toLocaleString(locale(), { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

/**
 * An amount as typed, or null if it isn't one (empty included). A leading "$" and spaces are ignored. With both
 * "." and "," the last one is the decimal separator ("1,250.00", "1.250,00"); commas each before exactly three
 * digits separate thousands ("1,250", "1,250,000"), a lone comma before one or two is the decimal point ("12,5").
 * The Rust CSV import reads prices the same way.
 */
export function parseAmount(text: string): number | null {
  let s = text.trim().replace(/^\$\s*/, "");
  if (s.includes(".") && s.includes(",")) {
    const decimal = s.lastIndexOf(".") > s.lastIndexOf(",") ? "." : ",";
    s = s.replaceAll(decimal === "." ? "," : ".", "").replace(",", ".");
  } else if (s.includes(",")) {
    const [head, ...groups] = s.split(",");
    if (!/^-?\d+$/.test(head)) return null;
    if (groups.length === 1 && /^\d{1,2}$/.test(groups[0])) s = `${head}.${groups[0]}`;
    else if (groups.every((g) => /^\d{3}$/.test(g))) s = head + groups.join("");
    else return null;
  }
  return /^-?(\d+\.?\d*|\.\d+)$/.test(s) ? Number(s) : null;
}

export function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function dateTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" });
}

/** "45s", "3m 20s", "1h 5m". */
export function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.round(seconds);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

/** A local file (in the app data dir) as an <img> URL. */
export function fileUrl(path: string | null | undefined): string | undefined {
  if (!path) return undefined;
  return path.startsWith("data:") ? path : convertFileSrc(path);
}

export function defaultSessionName(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(
    now.getMinutes(),
  )}`;
  return t("status.sessionName", { date });
}

/** Estimated seconds left from recent progress samples [ms timestamp, processed count]. */
export function eta(samples: [number, number][], remaining: number): number | null {
  if (samples.length < 2 || remaining <= 0) return null;
  const [t0, n0] = samples[0];
  const [t1, n1] = samples[samples.length - 1];
  const rate = (n1 - n0) / ((t1 - t0) / 1000);
  if (!(rate > 0)) return null;
  return remaining / rate;
}

/** "229 MB", "1.4 GB". */
export function bytes(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`;
  if (n >= 1e6) return `${Math.round(n / 1e6)} MB`;
  return `${Math.round(n / 1e3)} kB`;
}

// Functions, not constants: the labels follow the app's language.

export function engineLabels(): Record<Engine, string> {
  return { ximilar: t("status.engine.ximilar"), local: t("status.engine.local"), agent: t("status.engine.agent") };
}

export function idModeLabels(): Record<IdMode, string> {
  return {
    auto: t("status.idMode.auto"),
    tcg: t("status.idMode.tcg"),
    sport: t("status.idMode.sport"),
    comics: t("status.idMode.comics"),
    slab: t("status.idMode.slab"),
  };
}

export function taskLabels(): Record<Task, string> {
  return {
    identify: t("status.task.identify"),
    grade: t("status.task.grade"),
    condition: t("status.task.condition"),
    crop: t("status.task.crop"),
  };
}

/** What does the session's work. `engine: agent` means an agent only for identification: a session that doesn't
 * identify reads it as Ximilar (`SessionOptions::agent`). Prices and crops have their own (`pricesOf`,
 * `cropEngineOf`). */
export function engineOf(o: Pick<SessionOptions, "task" | "engine">): Engine {
  return o.engine === "agent" && o.task !== "identify" ? "ximilar" : o.engine;
}

/** Identified by an AI agent over MCP. */
export function agentOf(o: Pick<SessionOptions, "task" | "engine">): boolean {
  return engineOf(o) === "agent";
}

/** What grades the cards (`SessionOptions::grade_engine`): an agent never does, Ximilar grades for it. */
export function gradeEngineOf(o: Pick<SessionOptions, "engine" | "gradeEngine">): Engine {
  const engine = o.gradeEngine ?? o.engine;
  return engine === "agent" ? "ximilar" : engine;
}

/** What crops the cards (`SessionOptions::crop_engine`): Ximilar unless the session says otherwise, whatever
 * identifies. The local model or an AI agent gives the corners and Cardloader cuts the card. */
export function cropEngineOf(o: Pick<SessionOptions, "cropEngine">): Engine {
  return o.cropEngine ?? "ximilar";
}

/** The prices a session asks for, as chosen (an older session's `priceStats` means Ximilar's). */
export function chosenPrices(o: Pick<SessionOptions, "prices" | "priceStats">): Prices {
  return o.prices ?? (o.priceStats ? "ximilar" : "off");
}

/** Where the cards' prices come from (`SessionOptions::prices`): only identified cards are priced, and Ximilar's
 * price statistics come only with its identification. */
export function pricesOf(o: Pick<SessionOptions, "task" | "engine" | "prices" | "priceStats">): Prices {
  const prices = chosenPrices(o);
  if (o.task !== "identify") return "off";
  if (prices === "ximilar" && o.engine !== "ximilar") return "off";
  return prices;
}

/** What happens to each card, in order (`SessionOptions::stages`). */
export function stagesOf(o: SessionOptions): Stage[] {
  const prices = pricesOf(o);
  return [
    o.task === "identify" && ("identify" as const),
    (prices === "ebay" || prices === "agent") && ("price" as const),
    gradingOf(o) !== "off" && ("grade" as const),
    cropsOf(o) && ("crop" as const),
  ].filter((s): s is Stage => !!s);
}

/** Calls the Ximilar API for something (`SessionOptions::uses_api`): a session whose stages all run elsewhere (on
 * this computer, by an AI agent, on eBay) runs without a token. */
export function usesApi(o: SessionOptions): boolean {
  if (cropsOf(o) && cropEngineOf(o) === "ximilar") return true;
  if (o.task === "identify" && (o.syncRemote || o.engine === "ximilar")) return true;
  return gradingOf(o) !== "off" && gradeEngineOf(o) === "ximilar";
}

/** A task the New session dialog offers; several can be chosen. */
export type TaskChoice = "identify" | "prices" | "grade" | "crop";
export const TASK_CHOICES: TaskChoice[] = ["identify", "prices", "grade", "crop"];

/** The tasks a session's options do. */
export function tasksOf(o: SessionOptions): TaskChoice[] {
  const identifies = o.task === "identify";
  return TASK_CHOICES.filter((t) => {
    switch (t) {
      case "identify":
        return identifies;
      case "prices":
        return identifies && chosenPrices(o) !== "off";
      case "grade":
        return gradingOf(o) !== "off";
      case "crop":
        return cropsOf(o);
    }
  });
}

/** The options doing `tasks` (at least one), keeping what each task's tab chose: prices need identification, and
 * a session that doesn't identify is a grade, condition or crop session with crops as an extra. */
export function withTasks(o: SessionOptions, tasks: TaskChoice[]): SessionOptions {
  const has = (t: TaskChoice) => tasks.includes(t);
  const mode = gradingOf(o);
  // A grading mode carried over (a condition session that starts identifying), else a full grade.
  const grading: GradingMode = mode !== "off" ? mode : o.grading !== "off" ? o.grading : "grade";
  if (has("identify")) {
    const chosen = chosenPrices(o);
    const prices: Prices = !has("prices")
      ? "off"
      : chosen !== "off"
        ? chosen
        : o.engine === "ximilar"
          ? "ximilar"
          : "ebay";
    return {
      ...o,
      task: "identify",
      grading: has("grade") ? grading : "off",
      saveCrops: has("crop"),
      prices,
      priceStats: prices === "ximilar",
    };
  }
  if (has("grade")) {
    return {
      ...o,
      task: grading === "condition" ? "condition" : "grade",
      grading,
      saveCrops: has("crop"),
      syncRemote: false,
    };
  }
  return { ...o, task: "crop", saveCrops: false, syncRemote: false };
}

/** The grading a session's Grade tab chose: a full grade, centering or the condition, in a session that identifies
 * or one that only grades. */
export function withGradeMode(o: SessionOptions, mode: Exclude<GradingMode, "off">): SessionOptions {
  if (o.task === "identify") return { ...o, grading: mode };
  return { ...o, task: mode === "condition" ? "condition" : "grade", grading: mode };
}

/** The session saves cropped pictures of its cards (alone, or with what else it does). */
export function cropsOf(o: Pick<SessionOptions, "task" | "saveCrops">): boolean {
  return o.task === "crop" || o.saveCrops;
}

/** The card-grader endpoint a session's cards go to, as the runner reads it (`SessionOptions::grading_mode`). */
export function gradingOf(o: Pick<SessionOptions, "task" | "grading">): GradingMode {
  switch (o.task) {
    case "identify":
      return o.grading;
    case "grade":
      return o.grading === "centering" ? "centering" : "grade";
    case "condition":
      return "condition";
    case "crop":
      return "off";
  }
}

export function gradingLabels(): Record<GradingMode, string> {
  return {
    off: t("status.grading.off"),
    condition: t("status.grading.condition"),
    grade: t("status.grading.grade"),
    centering: t("status.grading.centering"),
  };
}

export function sourceLabels(): Record<SourceKind, string> {
  return {
    folder: t("status.source.folder"),
    watch_folder: t("status.source.watch_folder"),
    scanner: t("status.source.scanner"),
    csv: t("status.source.csv"),
  };
}
