import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useT } from "../i18n";
import { api } from "../lib/api";
import { SEARCH_FIELD, shortcutGroups, typing, useShortcuts, withMod } from "../lib/shortcuts";
import Modal from "./Modal";
import NewSessionDialog from "./NewSessionDialog";

const PAGES: Record<string, string> = { "1": "/", "2": "/sessions", "3": "/cards", ",": "/settings" };

/** The app-wide shortcuts (⌘ on macOS, Ctrl elsewhere), and what they open: the help and a new session. */
export function AppShortcuts() {
  const navigate = useNavigate();
  const { help, newSession, showHelp, createSession } = useShortcuts();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.repeat) return;
      // A dialog (or the setup) has the keyboard: only its own keys count.
      const dialog = !!document.querySelector("[role=dialog]");
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      if (withMod(e)) {
        if (key === "f") {
          // Never the webview's own find bar.
          e.preventDefault();
          if (!dialog) focusSearch();
          return;
        }
        if (dialog) return;
        if (key === "n") {
          e.preventDefault();
          api
            .getSettings()
            .then((s) => createSession(s.defaultSources[0] ?? "folder"))
            .catch(() => createSession("folder"));
        } else {
          // The digit keys by where they are: AZERTY layouts type "&" on the 1 key.
          const page = PAGES[e.code.startsWith("Digit") ? e.code.slice(5) : key];
          if (!page) return;
          e.preventDefault();
          navigate(page);
        }
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey || dialog || typing(e.target)) return;
      if (e.key === "?") {
        e.preventDefault();
        showHelp(true);
      } else if (e.key === "/" && focusSearch()) {
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, showHelp, createSession]);

  return (
    <>
      {help && <ShortcutsHelp onClose={() => showHelp(false)} />}
      {newSession && <NewSessionDialog initialKind={newSession} onClose={() => createSession(null)} />}
    </>
  );
}

/** Focuses the page's search field, if it has one. */
function focusSearch(): boolean {
  const field = document.querySelector<HTMLInputElement>(`[${Object.keys(SEARCH_FIELD)[0]}]`);
  if (!field) return false;
  field.focus();
  field.select();
  return true;
}

export function ShortcutsHelp({ onClose }: { onClose: () => void }) {
  const t = useT();
  return (
    <Modal title={t("session.keys.title")} width="max-w-md" onClose={onClose}>
      <ShortcutsList />
      <p className="mt-4 text-xs text-slate-500">{t("session.keys.tip")}</p>
    </Modal>
  );
}

/** Every shortcut, by where it works. */
export function ShortcutsList() {
  return (
    <div className="space-y-5">
      {shortcutGroups().map((group) => (
        <div key={group.title}>
          <h3 className="mb-2 text-xs font-medium tracking-wide text-slate-500 uppercase">{group.title}</h3>
          <dl className="grid grid-cols-[130px_1fr] gap-y-2 text-sm">
            {group.keys.map(([k, v]) => (
              <div key={k + v} className="contents">
                <dt>
                  <kbd className="rounded border border-slate-300 bg-slate-50 px-1.5 py-0.5 font-mono text-xs whitespace-nowrap dark:border-slate-700 dark:bg-slate-900">
                    {k}
                  </kbd>
                </dt>
                <dd className="text-slate-600 dark:text-slate-300">{v}</dd>
              </div>
            ))}
          </dl>
        </div>
      ))}
    </div>
  );
}
