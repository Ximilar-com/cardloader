// Choosing a session's cards for a marketplace and pricing each: the table the eBay, TCGplayer and Whatnot
// dialogs share.

import clsx from "clsx";
import type { ReactNode } from "react";
import type { CardCondition, EbayPreview } from "../lib/api";
import { fileUrl } from "../lib/format";
import { Check } from "./NewSessionDialog";
import type { SelectOption } from "./Select";

export const CONDITIONS: SelectOption<CardCondition>[] = [
  { value: "near_mint", label: "Near Mint or better" },
  { value: "excellent", label: "Excellent", hint: "lightly played" },
  { value: "very_good", label: "Very Good", hint: "moderately played" },
  { value: "poor", label: "Poor", hint: "heavily played" },
];

/** A price as typed, if it is one. */
export function parsePrice(text: string | undefined): number | null {
  const n = Number((text ?? "").replace(",", ".").trim());
  return text?.trim() && Number.isFinite(n) && n > 0 ? Math.round(n * 100) / 100 : null;
}

/** The note under a card: its category and condition, or why it can't go. */
export function defaultNote(c: EbayPreview): ReactNode {
  if (c.problem) return <span className="text-amber-700 dark:text-amber-400">{c.problem}</span>;
  return [c.category, c.condition].filter(Boolean).join(" · ");
}

export default function PricedCards({
  cards,
  chosen,
  onChosen,
  prices,
  onPrices,
  can,
  note = defaultNote,
  badge,
  verb,
  showPrices = true,
}: {
  cards: EbayPreview[];
  chosen: Set<number>;
  onChosen: (chosen: Set<number>) => void;
  prices: Record<number, string>;
  onPrices: (prices: Record<number, string>) => void;
  /** The card can be chosen. */
  can: (c: EbayPreview) => boolean;
  note?: (c: EbayPreview) => ReactNode;
  badge?: (c: EbayPreview) => ReactNode;
  /** "listed", "exported". */
  verb: string;
  showPrices?: boolean;
}) {
  const open = cards.filter(can);
  const all = open.length > 0 && open.every((c) => chosen.has(c.itemId));
  const toggle = (id: number, on: boolean) => {
    const next = new Set(chosen);
    if (on) next.add(id);
    else next.delete(id);
    onChosen(next);
  };
  return (
    <div className="rounded-md border border-slate-200 dark:border-slate-800">
      <div className="flex items-center gap-3 border-b border-slate-200 bg-slate-50 px-3 py-1.5 text-xs text-slate-500 dark:border-slate-800 dark:bg-slate-900">
        <Check
          label={all ? "All cards" : `${open.length} card${open.length === 1 ? "" : "s"} can be ${verb}`}
          checked={all}
          onChange={(on) => onChosen(on ? new Set(open.map((c) => c.itemId)) : new Set())}
        />
        <span className="flex-1" />
        {showPrices && <span className="w-28 text-right">Price (USD)</span>}
      </div>
      <ul className="max-h-[45vh] divide-y divide-slate-100 overflow-y-auto dark:divide-slate-900">
        {cards.map((c) => (
          <CardRow
            key={c.itemId}
            card={c}
            enabled={can(c)}
            chosen={chosen.has(c.itemId)}
            onChoose={(on) => toggle(c.itemId, on)}
            price={prices[c.itemId] ?? ""}
            onPrice={(v) => onPrices({ ...prices, [c.itemId]: v })}
            note={note(c)}
            badge={badge?.(c)}
            showPrice={showPrices}
          />
        ))}
      </ul>
    </div>
  );
}

function CardRow({
  card: c,
  enabled,
  chosen,
  onChoose,
  price: value,
  onPrice,
  note,
  badge,
  showPrice,
}: {
  card: EbayPreview;
  showPrice: boolean;
  enabled: boolean;
  chosen: boolean;
  onChoose: (on: boolean) => void;
  price: string;
  onPrice: (v: string) => void;
  note: ReactNode;
  badge?: ReactNode;
}) {
  const bad = showPrice && chosen && parsePrice(value) == null;
  return (
    <li className={clsx("flex items-center gap-3 px-3 py-2", !enabled && "opacity-60")}>
      <input
        type="checkbox"
        className="h-4 w-4 shrink-0 accent-[var(--color-accent)]"
        checked={chosen}
        disabled={!enabled}
        onChange={(e) => onChoose(e.target.checked)}
        aria-label={`Choose #${c.seq}`}
      />
      {c.thumbPath ? (
        <img src={fileUrl(c.thumbPath)} alt="" className="h-10 w-auto max-w-[30px] rounded object-contain" />
      ) : (
        <span className="h-10 w-[30px] rounded bg-slate-100 dark:bg-slate-800" />
      )}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="text-xs text-slate-400 tabular-nums">{c.seq}</span>
          <span className="truncate text-sm font-medium" title={c.title}>
            {c.title || "—"}
          </span>
          {badge}
        </div>
        <div className="truncate text-xs text-slate-500">{note}</div>
      </div>
      <div className={clsx("relative w-28 shrink-0", !showPrice && "hidden")}>
        <span className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-sm text-slate-400">$</span>
        <input
          className={clsx("input pl-6 text-right tabular-nums", bad && "border-red-400 ring-1 ring-red-300")}
          inputMode="decimal"
          value={value}
          placeholder="Price"
          disabled={!enabled}
          onChange={(e) => {
            onPrice(e.target.value);
            if (e.target.value.trim()) onChoose(true);
          }}
          aria-label={`Price of #${c.seq}`}
        />
      </div>
    </li>
  );
}
