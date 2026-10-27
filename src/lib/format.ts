import { convertFileSrc } from "@tauri-apps/api/core";
import { locale, t } from "../i18n";
import type { Engine, GradingMode, IdMode, SourceKind } from "./api";

/** Ximilar's prices are in US dollars, written the way the app's language writes amounts. */
export function price(value: number | null | undefined): string {
  return value == null ? "—" : new Intl.NumberFormat(locale(), { style: "currency", currency: "USD" }).format(value);
}

export function grade(value: number | null | undefined): string {
  if (value == null) return "—";
  return value.toLocaleString(locale(), { minimumFractionDigits: 0, maximumFractionDigits: 1 });
}

/** A number shown with `digits` decimals, as the app's language writes them ("0.120", "0,120"). */
export function decimal(value: number, digits: number): string {
  return value.toLocaleString(locale(), { minimumFractionDigits: digits, maximumFractionDigits: digits });
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
  return { ximilar: t("status.engine.ximilar"), local: t("status.engine.local") };
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
