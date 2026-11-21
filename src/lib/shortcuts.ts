import { create } from "zustand";
import { t } from "../i18n";
import type { SourceKind } from "./api";

/** macOS takes ⌘ where Windows and Linux take Ctrl. */
export const IS_MAC =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** The platform's command modifier alone: ⌘ on macOS, Ctrl elsewhere (never both, never with Alt). */
export function withMod(e: KeyboardEvent): boolean {
  if (e.altKey) return false;
  return IS_MAC ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** A shortcut with the command modifier as the platform writes it: "⌘N" on macOS, "Ctrl+N" elsewhere. */
export function modKey(key: string): string {
  return IS_MAC ? `⌘${key}` : `Ctrl+${key}`;
}

/** Keys typed into a field are text, not shortcuts. */
export function typing(target: EventTarget | null): boolean {
  return !!(target as HTMLElement | null)?.closest?.("input, textarea, select, [contenteditable=true]");
}

/** Marks the search field of a page: ⌘F / Ctrl+F and / focus it. */
export const SEARCH_FIELD = { "data-shortcut-search": "" } as const;

/** What the app-wide shortcuts open: the shortcuts help, and a new session (with the source it starts on). */
export const useShortcuts = create<{
  help: boolean;
  newSession: SourceKind | null;
  showHelp: (open: boolean) => void;
  createSession: (kind: SourceKind | null) => void;
}>((set) => ({
  help: false,
  newSession: null,
  showHelp: (help) => set({ help }),
  createSession: (newSession) => set({ newSession }),
}));

export interface ShortcutGroup {
  title: string;
  keys: [string, string][];
}

/** Every shortcut the app has, by where it works: listed in the help (?) and in Settings › Keyboard shortcuts. */
export function shortcutGroups(): ShortcutGroup[] {
  return [
    {
      title: t("settings.shortcuts.anywhere"),
      keys: [
        [modKey("N"), t("settings.shortcuts.newSession")],
        [modKey("1"), t("settings.shortcuts.home")],
        [modKey("2"), t("settings.shortcuts.sessions")],
        [modKey("3"), t("settings.shortcuts.cards")],
        [modKey(","), t("settings.shortcuts.settings")],
        [`${modKey("F")}  /  /`, t("settings.shortcuts.search")],
        ["?", t("settings.shortcuts.help")],
      ],
    },
    {
      title: t("settings.shortcuts.inSession"),
      keys: [
        ["J  /  ↓", t("session.keys.next")],
        ["K  /  ↑", t("session.keys.previous")],
        ["A", t("session.keys.accept")],
        ["1 – 9", t("session.keys.select")],
        ["E", t("session.keys.edit")],
        [t("settings.shortcuts.space"), t("settings.shortcuts.enlarge")],
        ["Delete", t("session.keys.delete")],
        ["Esc", t("session.keys.close")],
        [t("session.keys.shiftClick"), t("session.keys.range")],
      ],
    },
    {
      title: t("settings.shortcuts.picture"),
      keys: [
        ["←  /  →", t("settings.shortcuts.sides")],
        ["O", t("settings.shortcuts.original")],
        ["Esc", t("settings.shortcuts.closePicture")],
      ],
    },
  ];
}
