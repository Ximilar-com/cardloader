import clsx from "clsx";
import { MoreHorizontal, type LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

export interface MenuItem {
  label: string;
  icon?: LucideIcon;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
  /** Shown on hover, e.g. why the item is disabled. */
  title?: string;
}

/** A "⋯" button with a dropdown. Fixed positioning, so scrolling tables don't clip it. */
export default function Menu({
  items,
  label = "More actions",
}: {
  items: (MenuItem | null | false)[];
  label?: string;
}) {
  const [at, setAt] = useState<{ top: number; right: number; up: boolean } | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const list = items.filter(Boolean) as MenuItem[];

  useEffect(() => {
    if (!at) return;
    const close = () => setAt(null);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [at]);

  const toggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (at) return setAt(null);
    const r = button.current!.getBoundingClientRect();
    const up = r.bottom + list.length * 34 + 16 > window.innerHeight;
    setAt({ top: up ? r.top : r.bottom, right: window.innerWidth - r.right, up });
  };

  return (
    <>
      <button
        ref={button}
        className={clsx("btn btn-ghost px-1.5", at && "bg-slate-100 dark:bg-slate-800")}
        onClick={toggle}
        onMouseDown={(e) => e.stopPropagation()}
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={!!at}
        title={label}
      >
        <MoreHorizontal className="h-4 w-4" />
      </button>
      {at && (
        <div
          role="menu"
          className="card fixed z-50 min-w-48 py-1 shadow-lg"
          style={{ right: at.right, ...(at.up ? { bottom: window.innerHeight - at.top + 4 } : { top: at.top + 4 }) }}
          onMouseDown={(e) => e.stopPropagation()}
          onClick={(e) => e.stopPropagation()}
        >
          {list.map(({ label, icon: Icon, onClick, danger, disabled, title }) => (
            <button
              key={label}
              role="menuitem"
              disabled={disabled}
              title={title}
              onClick={() => {
                setAt(null);
                onClick();
              }}
              className={clsx(
                "flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm disabled:cursor-not-allowed disabled:opacity-40",
                danger
                  ? "text-red-600 enabled:hover:bg-red-50 dark:text-red-400 dark:enabled:hover:bg-red-950/50"
                  : "enabled:hover:bg-slate-100 dark:enabled:hover:bg-slate-800",
              )}
            >
              {Icon && <Icon className="h-4 w-4 shrink-0 opacity-70" />}
              {label}
            </button>
          ))}
        </div>
      )}
    </>
  );
}
