import clsx from "clsx";
import { AlertCircle, CheckCircle2, Clock, Loader2, Pause, Square, Star } from "lucide-react";
import type { ItemStatus, SessionStatus } from "../lib/api";

const STYLES: Record<string, string> = {
  draft: "bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300",
  running: "bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300",
  paused: "bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300",
  stopped: "bg-slate-200 text-slate-700 dark:bg-slate-800 dark:text-slate-300",
  completed: "bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300",
  failed: "bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300",
  queued: "bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400",
  recognizing: "bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300",
  grading: "bg-violet-100 text-violet-700 dark:bg-violet-950 dark:text-violet-300",
  done: "bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300",
  error: "bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300",
  live: "bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300",
  closed: "bg-slate-200 text-slate-700 dark:bg-slate-800 dark:text-slate-300",
};

const ICONS: Record<string, typeof Clock> = {
  running: Loader2,
  recognizing: Loader2,
  grading: Star,
  paused: Pause,
  stopped: Square,
  completed: CheckCircle2,
  done: CheckCircle2,
  failed: AlertCircle,
  error: AlertCircle,
  queued: Clock,
};

export default function StatusChip({ status }: { status: SessionStatus | ItemStatus | "live" | "closed" }) {
  const Icon = ICONS[status];
  const spin = status === "running" || status === "recognizing";
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium capitalize",
        STYLES[status],
      )}
    >
      {Icon && <Icon className={clsx("h-3 w-3", spin && "animate-spin")} />}
      {status}
    </span>
  );
}

export function ConfidenceChip({ value }: { value: string | null }) {
  if (!value) return <span className="text-slate-400">—</span>;
  if (value === "local")
    return (
      <span
        className="rounded bg-slate-100 px-1.5 py-0.5 text-[11px] font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300"
        title="Read by the local model on this computer: no match score"
      >
        Local
      </span>
    );
  const style =
    value === "high"
      ? "text-emerald-700 bg-emerald-50 dark:bg-emerald-950 dark:text-emerald-300"
      : value === "medium"
        ? "text-amber-700 bg-amber-50 dark:bg-amber-950 dark:text-amber-300"
        : "text-red-700 bg-red-50 dark:bg-red-950 dark:text-red-300";
  return <span className={clsx("rounded px-1.5 py-0.5 text-[11px] font-medium capitalize", style)}>{value}</span>;
}

export function PickedChip() {
  return (
    <span
      className="rounded bg-accent/10 px-1.5 py-0.5 text-[11px] font-medium text-accent dark:bg-accent/25 dark:text-blue-300"
      title="You picked this match from the candidates"
    >
      Picked
    </span>
  );
}

export function EditedChip() {
  return (
    <span
      className="rounded bg-accent/10 px-1.5 py-0.5 text-[11px] font-medium text-accent dark:bg-accent/25 dark:text-blue-300"
      title="You corrected this card by hand"
    >
      Edited
    </span>
  );
}

/** An unsure match nobody has looked at: a small dot next to the confidence. */
export function ReviewDot() {
  return (
    <span
      className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500"
      title="To review: accept the match (A) or pick another candidate"
      aria-label="To review"
    />
  );
}
