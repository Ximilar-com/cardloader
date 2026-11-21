import { X } from "lucide-react";
import { useEffect, useRef, type MouseEvent, type ReactNode } from "react";
import { useT } from "../i18n";

/** When the window last got the focus back, e.g. from a native folder picker. */
let refocusedAt = 0;
if (typeof window !== "undefined") window.addEventListener("focus", () => (refocusedAt = Date.now()));

/** Closing on a click beside a dialog: only a whole click there (pressed and released on the backdrop), and not
 * one in the moment the window gets the focus back. On macOS the click that closes a native picker (Browse →
 * Open) can reach the page beneath it, which closed the dialog it was picking for. */
export function useBackdropClose(onClose: () => void) {
  const pressed = useRef(false);
  return {
    onMouseDown: (e: MouseEvent) => {
      pressed.current = e.target === e.currentTarget && Date.now() - refocusedAt > 500;
    },
    onClick: (e: MouseEvent) => {
      if (pressed.current && e.target === e.currentTarget) onClose();
      pressed.current = false;
    },
  };
}

export default function Modal({
  title,
  onClose,
  children,
  footer,
  width = "max-w-2xl",
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
}) {
  const t = useT();
  const backdrop = useBackdropClose(onClose);
  useEffect(() => {
    // Not an Escape that a control inside already took, such as an open Select closing its list.
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !e.defaultPrevented && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/40 p-6" {...backdrop}>
      <div
        className={`card flex max-h-full w-full ${width} flex-col shadow-2xl`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="flex items-center justify-between border-b border-slate-200 px-5 py-3 dark:border-slate-800">
          <h2 className="text-base font-semibold">{title}</h2>
          <button className="btn btn-ghost px-1.5" onClick={onClose} aria-label={t("common.close")}>
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{children}</div>
        {footer && (
          <div className="flex justify-end gap-2 border-t border-slate-200 px-5 py-3 dark:border-slate-800">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}

export function ErrorNote({ children }: { children: ReactNode }) {
  if (!children) return null;
  return (
    <div className="selectable rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/50 dark:text-red-300">
      {children}
    </div>
  );
}
