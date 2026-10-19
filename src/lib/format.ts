import { convertFileSrc } from "@tauri-apps/api/core";
import type { GradingMode, IdMode } from "./api";

const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });

export function price(value: number | null | undefined): string {
  return value == null ? "—" : usd.format(value);
}

export function grade(value: number | null | undefined): string {
  if (value == null) return "—";
  return Number.isInteger(value) ? value.toFixed(0) : value.toFixed(1);
}

export function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function dateTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
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
  return `Session ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(
    now.getMinutes(),
  )}`;
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

export const ENGINE_LABELS: Record<string, string> = {
  ximilar: "Ximilar API",
  local: "Local model",
};

export const ID_MODE_LABELS: Record<IdMode, string> = {
  auto: "Auto-detect",
  tcg: "Trading card games",
  sport: "Sport cards",
  comics: "Comics",
  slab: "Graded slabs",
};

export const GRADING_LABELS: Record<GradingMode, string> = {
  off: "Off",
  condition: "Condition (fast)",
  grade: "Full grade",
  centering: "Centering only",
};

export const SOURCE_LABELS: Record<string, string> = {
  folder: "Folder",
  watch_folder: "Watch folder",
  scanner: "Scanner",
  csv: "CSV file",
};
