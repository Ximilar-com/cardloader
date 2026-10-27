import clsx from "clsx";
import {
  AlertCircle,
  Bot,
  CircleCheck,
  CircleQuestionMark,
  CheckCircle2,
  Clock,
  Loader2,
  Pause,
  Square,
  Star,
} from "lucide-react";
import { useT, type Key } from "../i18n";
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

const ITEM_STATUSES = new Set<string>(["queued", "recognizing", "grading", "done", "error"]);

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
  const t = useT();
  const spin = status === "running" || status === "recognizing";
  const label: Key = ITEM_STATUSES.has(status)
    ? `status.item.${status as ItemStatus}`
    : `status.session.${status as SessionStatus | "live" | "closed"}`;
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium",
        STYLES[status],
      )}
    >
      {Icon && <Icon className={clsx("h-3 w-3", spin && "animate-spin")} />}
      {t(label)}
    </span>
  );
}

export function ConfidenceChip({ value }: { value: string | null }) {
  const t = useT();
  if (!value) return <span className="text-slate-400">—</span>;
  if (value === "local")
    return (
      <span
        className="rounded bg-slate-100 px-1.5 py-0.5 text-[11px] font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300"
        title={t("status.confidence.localTitle")}
      >
        {t("status.confidence.local")}
      </span>
    );
  const style =
    value === "high"
      ? "text-emerald-700 bg-emerald-50 dark:bg-emerald-950 dark:text-emerald-300"
      : value === "medium"
        ? "text-amber-700 bg-amber-50 dark:bg-amber-950 dark:text-amber-300"
        : "text-red-700 bg-red-50 dark:bg-red-950 dark:text-red-300";
  const label = value === "high" || value === "medium" || value === "low" ? t(`status.confidence.${value}`) : value;
  return <span className={clsx("rounded px-1.5 py-0.5 text-[11px] font-medium capitalize", style)}>{label}</span>;
}

export function PickedChip() {
  const t = useT();
  return (
    <span
      className="rounded bg-accent/10 px-1.5 py-0.5 text-[11px] font-medium text-accent dark:bg-accent/25 dark:text-blue-300"
      title={t("status.pickedTitle")}
    >
      {t("status.picked")}
    </span>
  );
}

export function EditedChip() {
  const t = useT();
  return (
    <span
      className="rounded bg-accent/10 px-1.5 py-0.5 text-[11px] font-medium text-accent dark:bg-accent/25 dark:text-blue-300"
      title={t("status.editedTitle")}
    >
      {t("status.edited")}
    </span>
  );
}

/** An identified card nobody has accepted or corrected yet. */
export function ReviewMark() {
  const t = useT();
  return (
    <span className="shrink-0" title={t("status.toReviewTitle")} aria-label={t("status.toReview")}>
      <CircleQuestionMark className="h-[18px] w-[18px] text-orange-500" strokeWidth={2.5} />
    </span>
  );
}

export function ReviewChip() {
  const t = useT();
  return (
    <span
      className="inline-flex items-center gap-1 rounded bg-orange-50 px-1.5 py-0.5 text-[11px] font-medium text-orange-700 dark:bg-orange-950 dark:text-orange-300"
      title={t("status.toReviewChipTitle")}
    >
      <CircleQuestionMark className="h-3 w-3" /> {t("status.toReview")}
    </span>
  );
}

/** The user accepted the match: a green check next to the confidence. */
export function AcceptedMark() {
  const t = useT();
  return (
    <span className="shrink-0" title={t("status.acceptedTitle")} aria-label={t("status.accepted")}>
      <CircleCheck className="h-[18px] w-[18px] text-emerald-600 dark:text-emerald-400" strokeWidth={2.5} />
    </span>
  );
}

/** An AI agent accepted, picked or corrected the card (over MCP): the user may want to check it. */
export function AgentMark({ note }: { note?: string }) {
  const t = useT();
  const title = note ? t("status.agentReviewedNote", { note }) : t("status.agentReviewed");
  return (
    <span className="shrink-0" title={title} aria-label={t("status.reviewedByAi")}>
      <Bot className="h-[18px] w-[18px] text-indigo-500 dark:text-indigo-400" strokeWidth={2.25} />
    </span>
  );
}

export function AgentChip({ note }: { note?: string }) {
  const t = useT();
  return (
    <span
      className="inline-flex items-center gap-1 rounded bg-indigo-50 px-1.5 py-0.5 text-[11px] font-medium text-indigo-700 dark:bg-indigo-950 dark:text-indigo-300"
      title={note ? t("status.agentReviewedNote", { note }) : t("status.agentReviewed")}
    >
      <Bot className="h-3 w-3" /> {t("status.ai")}
    </span>
  );
}

export function AcceptedChip() {
  const t = useT();
  return (
    <span
      className="inline-flex items-center gap-1 rounded bg-emerald-50 px-1.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300"
      title={t("status.acceptedTitle")}
    >
      <CircleCheck className="h-3 w-3" /> {t("status.accepted")}
    </span>
  );
}
