import clsx from "clsx";
import type { SessionCounts } from "../lib/api";

/** Done (green), errors (red), grading (violet), in progress (blue) out of the total. */
export default function ProgressBar({ counts, active }: { counts: SessionCounts; active: boolean }) {
  const total = Math.max(counts.total, 1);
  const pct = (n: number) => `${(n / total) * 100}%`;
  return (
    <div className="flex h-2.5 w-full overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
      <div className="bg-emerald-500 transition-all duration-500" style={{ width: pct(counts.done) }} />
      <div className="bg-violet-500 transition-all duration-500" style={{ width: pct(counts.grading) }} />
      <div
        className={clsx("bg-accent transition-all duration-500", active && "progress-stripes")}
        style={{ width: pct(counts.inProgress) }}
      />
      <div className="bg-red-500 transition-all duration-500" style={{ width: pct(counts.errors) }} />
    </div>
  );
}
