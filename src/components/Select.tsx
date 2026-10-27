// The app's select: a button showing the chosen option and a list of options with icons, hints and badges.
// The list is fixed-positioned, so dialogs and scrolling panels don't clip it.

import clsx from "clsx";
import { Check, ChevronDown, type LucideIcon } from "lucide-react";
import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useT } from "../i18n";

export interface SelectOption<T extends string> {
  value: T;
  label: ReactNode;
  /** For type-ahead when `label` isn't plain text. */
  text?: string;
  icon?: LucideIcon;
  /** Secondary text after the label, e.g. what the option is for. */
  hint?: string;
  /** Shown at the right end, e.g. a status badge. */
  badge?: ReactNode;
  disabled?: boolean;
}

const LIST_MAX_HEIGHT = 288;

export default function Select<T extends string>({
  value,
  options,
  onChange,
  disabled,
  placeholder,
  className,
  "aria-label": ariaLabel,
}: {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
  "aria-label"?: string;
}) {
  const t = useT();
  const [at, setAt] = useState<{ left: number; width: number; top?: number; bottom?: number } | null>(null);
  const [active, setActive] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const typed = useRef({ text: "", at: 0 });
  const id = useId();
  const selected = options.find((o) => o.value === value);
  const open = !!at;

  useEffect(() => {
    if (!at) return;
    const close = (e: Event) => {
      const target = e.target as Node;
      if (!list.current?.contains(target) && !button.current?.contains(target)) setAt(null);
    };
    const onResize = () => setAt(null);
    window.addEventListener("mousedown", close);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", onResize);
    };
  }, [at]);

  useEffect(() => {
    if (open) list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  const show = () => {
    const r = button.current!.getBoundingClientRect();
    const below = window.innerHeight - r.bottom;
    const up = below < Math.min(LIST_MAX_HEIGHT, options.length * 36 + 8) + 8 && r.top > below;
    setAt({
      left: r.left,
      width: r.width,
      ...(up ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }),
    });
    setActive(Math.max(0, options.indexOf(selected!)));
  };
  const pick = (o: SelectOption<T> | undefined) => {
    if (!o || o.disabled) return;
    setAt(null);
    if (o.value !== value) onChange(o.value);
    button.current?.focus();
  };
  const move = (from: number, step: number) => {
    for (let i = from + step; i >= 0 && i < options.length; i += step) {
      if (!options[i].disabled) return setActive(i);
    }
  };

  const onKey = (e: KeyboardEvent) => {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    if (e.key === "Escape" || e.key === "Tab") {
      if (e.key === "Escape") e.preventDefault();
      setAt(null);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      move(active, e.key === "ArrowDown" ? 1 : -1);
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      move(e.key === "Home" ? -1 : options.length, e.key === "Home" ? 1 : -1);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pick(options[active]);
    } else if (e.key.length === 1) {
      // Type-ahead: jump to the first option starting with what was typed.
      const now = Date.now();
      typed.current = { text: (now - typed.current.at < 700 ? typed.current.text : "") + e.key.toLowerCase(), at: now };
      const i = options.findIndex((o) => !o.disabled && textOf(o).toLowerCase().startsWith(typed.current.text));
      if (i >= 0) setActive(i);
    }
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        className={clsx("input flex items-center gap-2 text-left", disabled && "cursor-not-allowed", className)}
        disabled={disabled}
        onClick={() => (open ? setAt(null) : show())}
        onKeyDown={onKey}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-label={ariaLabel}
      >
        {selected ? (
          <Line option={selected} />
        ) : (
          <span className="flex-1 text-slate-400">{placeholder ?? t("common.select")}</span>
        )}
        <ChevronDown className={clsx("h-4 w-4 shrink-0 text-slate-400 transition-transform", open && "rotate-180")} />
      </button>
      {at && (
        <div
          ref={list}
          id={id}
          role="listbox"
          className="card fixed z-50 overflow-y-auto py-1 shadow-lg"
          style={{
            left: at.left,
            minWidth: at.width,
            maxWidth: Math.max(at.width, window.innerWidth - at.left - 8),
            maxHeight: LIST_MAX_HEIGHT,
            top: at.top,
            bottom: at.bottom,
          }}
        >
          {options.map((o, i) => (
            <div
              key={o.value}
              data-index={i}
              role="option"
              aria-selected={o.value === value}
              aria-disabled={o.disabled}
              className={clsx(
                "flex cursor-default items-center gap-2 px-2.5 py-1.5 text-sm",
                o.disabled ? "opacity-50" : i === active && "bg-slate-100 dark:bg-slate-800",
              )}
              onMouseEnter={() => !o.disabled && setActive(i)}
              onClick={() => pick(o)}
            >
              <Check className={clsx("h-4 w-4 shrink-0 text-accent", o.value !== value && "invisible")} />
              <Line option={o} />
            </div>
          ))}
        </div>
      )}
    </>
  );
}

function Line<T extends string>({ option: o }: { option: SelectOption<T> }) {
  const Icon = o.icon;
  return (
    <span className="flex min-w-0 flex-1 items-center gap-2">
      {Icon && <Icon className="h-4 w-4 shrink-0 text-slate-500 dark:text-slate-400" />}
      {/* The label keeps its room; a hint that doesn't fit is cut first (long languages). */}
      <span className="max-w-full shrink-0 truncate">{o.label}</span>
      {o.hint && <span className="min-w-0 truncate text-xs text-slate-400">{o.hint}</span>}
      {o.badge && (
        <>
          <span className="flex-1" />
          <span className="shrink-0">{o.badge}</span>
        </>
      )}
    </span>
  );
}

function textOf<T extends string>(o: SelectOption<T>): string {
  return o.text ?? (typeof o.label === "string" ? o.label : "");
}

/** A small rounded label for `badge`. */
export function Badge({ tone = "slate", children }: { tone?: "slate" | "green" | "sky"; children: ReactNode }) {
  return (
    <span
      className={clsx(
        "rounded px-1.5 py-0.5 text-[11px] font-medium tabular-nums",
        tone === "green" && "bg-emerald-50 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300",
        tone === "sky" && "bg-sky-50 text-sky-700 dark:bg-sky-950 dark:text-sky-300",
        tone === "slate" && "bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300",
      )}
    >
      {children}
    </span>
  );
}

/** Options from a `{ value: label }` record, e.g. idModeLabels(). */
export function optionsOf<T extends string>(labels: Record<T, string>): SelectOption<T>[] {
  return (Object.entries(labels) as [T, string][]).map(([value, label]) => ({ value, label }));
}
