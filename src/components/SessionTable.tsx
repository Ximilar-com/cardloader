import { useVirtualizer } from "@tanstack/react-virtual";
import clsx from "clsx";
import { ArrowDown, ArrowUp, Cloud, ImageOff } from "lucide-react";
import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { useT } from "../i18n";
import type { SlabLabel } from "../lib/api";
import type { Row, Sort, SortKey } from "../lib/cardview";
import { grade, price } from "../lib/format";
import { EbayBadge } from "./Ebay";
import StatusChip, { AcceptedMark, AgentMark, ConfidenceChip, EditedChip, PickedChip, ReviewMark } from "./StatusChip";

/** Card and set take the spare width, unless conditions or slabs need it: they are words, not a number. */
function gridColumns(showSlab: boolean, wordy: boolean, checks: boolean): string {
  const [card, set, grade] = wordy
    ? ["minmax(110px,1.2fr)", "minmax(100px,1fr)", "minmax(68px,0.6fr)"]
    : ["minmax(130px,1.5fr)", "minmax(110px,1.2fr)", "52px"];
  const slab = showSlab ? ["minmax(72px,0.7fr)"] : [];
  return [...(checks ? ["16px"] : []), "44px", "32px", card, set, "76px", "84px", grade, ...slab, "136px"].join(" ");
}
const ROW_HEIGHT = 56;

export default function SessionTable({
  rows,
  selected,
  onSelect,
  fresh,
  follow,
  showStatus = true,
  showSlab = false,
  checked,
  onCheck,
  onCheckAll,
  sort,
  onSort,
}: {
  rows: Row[];
  selected: string | null;
  onSelect: (key: string) => void;
  /** Rows that just arrived: they animate in. */
  fresh?: Set<string>;
  /** Keep the newest row in view. */
  follow?: boolean;
  showStatus?: boolean;
  /** A column for the graded slab's company and grade. */
  showSlab?: boolean;
  /** Rows ticked for a bulk action; with `onCheck`, a checkbox column. Shift extends a range. */
  checked?: Set<string>;
  onCheck?: (key: string, range: boolean) => void;
  onCheckAll?: () => void;
  /** With `onSort`, the headers sort the rows (the caller orders them). */
  sort?: Sort | null;
  onSort?: (key: SortKey) => void;
}) {
  const t = useT();
  const wordy = useMemo(() => showSlab || rows.some((r) => r.grade == null && r.condition), [showSlab, rows]);
  const checks = !!onCheck;
  const columns = { gridTemplateColumns: gridColumns(showSlab, wordy, checks) };
  const allChecked = !!checked && rows.length > 0 && rows.every((r) => checked.has(r.key));
  const parent = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parent.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  useEffect(() => {
    if (follow && rows.length) virtualizer.scrollToIndex(rows.length - 1, { align: "end" });
  }, [follow, rows.length, virtualizer]);

  // Arrow keys (and J/K) move the selection.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const down = e.key === "ArrowDown" || e.key === "j";
      if (!(down || e.key === "ArrowUp" || e.key === "k") || !rows.length) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if ((e.target as HTMLElement)?.closest("input, textarea, select, [role=dialog]")) return;
      e.preventDefault();
      const i = rows.findIndex((r) => r.key === selected);
      const next = i < 0 ? 0 : Math.min(rows.length - 1, Math.max(0, i + (down ? 1 : -1)));
      onSelect(rows[next].key);
      virtualizer.scrollToIndex(next);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rows, selected, onSelect, virtualizer]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div
        style={columns}
        className="grid shrink-0 items-center gap-3 border-b border-slate-200 bg-slate-50 px-4 py-2 text-[11px] font-medium tracking-wide text-slate-500 uppercase dark:border-slate-800 dark:bg-slate-900"
      >
        {checks && (
          <input
            type="checkbox"
            className="h-4 w-4 accent-[var(--color-accent)]"
            checked={allChecked}
            onChange={() => onCheckAll?.()}
            aria-label={t("sessions.table.selectAll")}
          />
        )}
        <span />
        <Header k="seq" sort={sort} onSort={onSort}>
          #
        </Header>
        <Header k="name" sort={sort} onSort={onSort}>
          {t("sessions.table.card")}
        </Header>
        <Header k="set" sort={sort} onSort={onSort}>
          {t("sessions.table.setNo")}
        </Header>
        <Header k="confidence" sort={sort} onSort={onSort}>
          {t("sessions.table.match")}
        </Header>
        <Header k="price" sort={sort} onSort={onSort} right>
          {t("sessions.table.price")}
        </Header>
        <Header k="grade" sort={sort} onSort={onSort} right>
          {t("sessions.table.grade")}
        </Header>
        {showSlab && <span className="pl-2">{t("sessions.table.slab")}</span>}
        <span>{showStatus ? t("sessions.table.status") : ""}</span>
      </div>
      <div ref={parent} className="min-h-0 flex-1 overflow-y-auto">
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((v) => {
            const r = rows[v.index];
            return (
              <div
                key={r.key}
                onClick={() => onSelect(r.key)}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  right: 0,
                  height: v.size,
                  transform: `translateY(${v.start}px)`,
                  ...columns,
                }}
                className={clsx(
                  "grid cursor-pointer items-center gap-3 border-b border-slate-100 px-4 text-sm dark:border-slate-900",
                  r.key === selected
                    ? "bg-accent/10 dark:bg-accent/20"
                    : checked?.has(r.key)
                      ? "bg-accent/5 dark:bg-accent/10"
                      : "hover:bg-slate-50 dark:hover:bg-slate-900",
                  fresh?.has(r.key) && "row-new",
                )}
              >
                {checks && (
                  <input
                    type="checkbox"
                    className="h-4 w-4 accent-[var(--color-accent)]"
                    checked={checked?.has(r.key) ?? false}
                    onClick={(e) => {
                      e.stopPropagation();
                      onCheck?.(r.key, e.shiftKey);
                    }}
                    onChange={() => {}}
                    aria-label={t("sessions.table.selectCard", { seq: r.seq })}
                  />
                )}
                <Thumb src={r.thumb} />
                <span className="text-xs text-slate-400 tabular-nums">{r.seq}</span>
                <span className="min-w-0">
                  <span className="flex min-w-0 items-center gap-1.5 font-medium" title={r.fullName}>
                    <span className="truncate">
                      {r.name ||
                        (r.status === "done" ? (
                          <span className="text-slate-400">{t("status.noMatch")}</span>
                        ) : (
                          <span className="text-slate-400">—</span>
                        ))}
                    </span>
                    {r.copies != null && r.copies > 1 && (
                      <span
                        className="shrink-0 rounded bg-slate-100 px-1 text-[11px] text-slate-600 tabular-nums dark:bg-slate-800 dark:text-slate-300"
                        title={t("sessions.table.copies", { count: r.copies })}
                      >
                        ×{r.copies}
                      </span>
                    )}
                  </span>
                  {r.sessionName ? (
                    <span className="block truncate text-xs text-slate-400">
                      {r.sessionName}
                      {r.location && ` · ${r.location}`}
                    </span>
                  ) : r.error ? (
                    <span className="block truncate text-xs text-red-600 dark:text-red-400" title={r.error}>
                      {r.error}
                    </span>
                  ) : (
                    r.idType && (
                      <span className="block text-[10px] font-medium tracking-wide text-slate-400 uppercase">
                        {r.idType}
                      </span>
                    )
                  )}
                </span>
                <span className="min-w-0 text-slate-600 dark:text-slate-300">
                  <span className="block truncate" title={r.setName}>
                    {r.setName || "—"}
                  </span>
                  {r.cardNumber && <span className="block truncate text-xs text-slate-400">#{r.cardNumber}</span>}
                </span>
                <span className="flex items-center gap-1.5">
                  {r.edited ? <EditedChip /> : r.picked ? <PickedChip /> : <ConfidenceChip value={r.confidence} />}
                  {r.toReview && <ReviewMark />}
                  {r.byAgent ? <AgentMark note={r.reviewNote} /> : r.accepted && <AcceptedMark />}
                </span>
                <span className="text-right tabular-nums">{price(r.price)}</span>
                <span className="text-right tabular-nums" title={r.condition ?? undefined}>
                  {r.grade != null ? (
                    grade(r.grade)
                  ) : r.condition ? (
                    <span className="text-xs">{r.condition}</span>
                  ) : (
                    "—"
                  )}
                </span>
                {showSlab && <SlabCell slab={r.slab} />}
                <span className="flex items-center gap-1.5">
                  {r.status && <StatusChip status={r.status} />}
                  {r.synced && <Cloud className="h-3.5 w-3.5 text-slate-400" aria-label={t("sessions.table.synced")} />}
                  <EbayBadge listing={r.ebay} />
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function Header({
  k,
  sort,
  onSort,
  right,
  children,
}: {
  k: SortKey;
  sort?: Sort | null;
  onSort?: (key: SortKey) => void;
  right?: boolean;
  children: ReactNode;
}) {
  const t = useT();
  if (!onSort) return <span className={clsx(right && "text-right")}>{children}</span>;
  const active = sort?.key === k;
  const Arrow = sort?.desc ? ArrowDown : ArrowUp;
  return (
    <button
      className={clsx(
        "flex items-center gap-0.5 uppercase hover:text-slate-800 dark:hover:text-slate-200",
        right && "justify-end",
        active && "text-slate-800 dark:text-slate-200",
      )}
      onClick={() => onSort(k)}
      title={t("sessions.table.sort")}
    >
      {children}
      {active && <Arrow className="h-3 w-3" />}
    </button>
  );
}

function SlabCell({ slab }: { slab?: SlabLabel | null }) {
  const t = useT();
  if (!slab) return <span className="pl-2 text-slate-400">—</span>;
  return (
    <span
      className="min-w-0 pl-2"
      title={[
        slab.company,
        slab.grade,
        slab.certificate && t("sessions.table.certificate", { number: slab.certificate }),
      ]
        .filter(Boolean)
        .join(" · ")}
    >
      <span className="block truncate font-medium">{slab.company || t("sessions.table.slab")}</span>
      {slab.grade && <span className="block truncate text-xs text-slate-400 tabular-nums">{slab.grade}</span>}
    </span>
  );
}

function Thumb({ src }: { src?: string }) {
  if (!src)
    return (
      <span className="flex h-11 w-8 items-center justify-center rounded bg-slate-100 dark:bg-slate-800">
        <ImageOff className="h-3.5 w-3.5 text-slate-400" />
      </span>
    );
  return <img src={src} alt="" loading="lazy" className="h-11 w-auto max-w-[40px] rounded object-contain shadow-xs" />;
}
