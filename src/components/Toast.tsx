// One short message at a time in the corner, from anywhere: `toast("Exported 12 cards")`.

import { useEffect, useSyncExternalStore } from "react";
import { api, errorText } from "../lib/api";

interface Notice {
  id: number;
  text: string;
  error: boolean;
}

let current: Notice | null = null;
const listeners = new Set<() => void>();
let next = 1;

function set(notice: Notice | null) {
  current = notice;
  listeners.forEach((l) => l());
}

export function toast(text: string) {
  set({ id: next++, text, error: false });
}

/** Turns the upside-down cards among `ids` upright and says how many (the table doesn't know which ones are). */
export function turnUpright(ids: number[]) {
  return api
    .turnItems(ids)
    .then(({ turned, skipped }) =>
      toast(
        turned === 0
          ? "None of these cards is upside down"
          : `Turned ${turned} card${turned === 1 ? "" : "s"} upright${skipped ? ` · ${skipped} weren’t upside down` : ""}`,
      ),
    )
    .catch(toastError);
}

export function toastError(e: unknown) {
  set({ id: next++, text: errorText(e), error: true });
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export default function Toaster() {
  const notice = useSyncExternalStore(subscribe, () => current);
  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => current?.id === notice.id && set(null), notice.error ? 8000 : 4000);
    return () => clearTimeout(t);
  }, [notice]);
  if (!notice) return null;
  return (
    <div
      className={
        "selectable fixed right-6 bottom-6 z-50 max-w-md rounded-lg px-4 py-2.5 text-sm shadow-lg " +
        (notice.error ? "bg-red-600 text-white" : "bg-slate-900 text-white dark:bg-slate-100 dark:text-slate-900")
      }
      role="status"
      onClick={() => set(null)}
    >
      {notice.text}
    </div>
  );
}
