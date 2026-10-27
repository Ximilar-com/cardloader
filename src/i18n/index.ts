// The app's language: English dictionaries are the source (en/), every other language mirrors them key for key,
// which tsc checks through `Messages`. `t` reads the current language; components call `useT` so a change
// re-renders them.

import { createElement, Fragment, type ReactNode } from "react";
import { create } from "zustand";
import de from "./de";
import en from "./en";
import es from "./es";
import fr from "./fr";
import it from "./it";
import ja from "./ja";
import ko from "./ko";
import pt from "./pt";

/** A text that depends on `count`: the forms `Intl.PluralRules` names; `other` is required. */
export interface Plural {
  zero?: string;
  one?: string;
  two?: string;
  few?: string;
  many?: string;
  other: string;
}

/** A translation of an English dictionary: the same keys, any text. */
export type Messages<T> = {
  [K in keyof T]: T[K] extends string ? string : T[K] extends { other: string } ? Plural : Messages<T[K]>;
};

type Leaves<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string | { other: string } ? `${P}${K}` : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];

export type Key = Leaves<typeof en>;
export type Vars = Record<string, string | number>;

export type Language = "en" | "ja" | "es" | "ko" | "fr" | "pt" | "de" | "it";

export const DICTIONARIES: Record<Language, Messages<typeof en>> = { en, ja, es, ko, fr, pt, de, it };

/** The tag `Intl` formats dates and numbers with. */
const LOCALES: Record<Language, string> = {
  en: "en-US",
  ja: "ja-JP",
  es: "es-ES",
  ko: "ko-KR",
  fr: "fr-FR",
  pt: "pt-BR",
  de: "de-DE",
  it: "it-IT",
};

/** A stored code as a supported language; anything else is English. */
export function languageOf(code: string | null | undefined): Language {
  return code && code in DICTIONARIES ? (code as Language) : "en";
}

export const useI18n = create<{ language: Language; setLanguage: (code: string) => void }>((set) => ({
  language: "en",
  setLanguage: (code) => {
    const language = languageOf(code);
    if (typeof document !== "undefined") document.documentElement.lang = language;
    set({ language });
  },
}));

export function locale(): string {
  return LOCALES[useI18n.getState().language];
}

function lookup(dict: unknown, key: string): string | Plural | undefined {
  let node = dict;
  for (const part of key.split(".")) {
    if (node == null || typeof node !== "object") return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return node as string | Plural | undefined;
}

const pluralRules = new Map<string, Intl.PluralRules>();

function pluralForm(tag: string, count: number): keyof Plural {
  let rules = pluralRules.get(tag);
  if (!rules) pluralRules.set(tag, (rules = new Intl.PluralRules(tag)));
  return rules.select(count) as keyof Plural;
}

/** The text of `key` in the current language, `{name}`s filled from `vars` (numbers formatted for the language). */
export function t(key: Key, vars?: Vars): string {
  const language = useI18n.getState().language;
  const tag = LOCALES[language];
  const value = lookup(DICTIONARIES[language], key) ?? lookup(en, key) ?? key;
  let text: string;
  if (typeof value === "string") text = value;
  else {
    const count = Number(vars?.count ?? 0);
    text = value[pluralForm(tag, count)] ?? value.other;
  }
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const v = vars[name];
    if (v == null) return whole;
    return typeof v === "number" ? v.toLocaleString(tag) : v;
  });
}

/** `t` for components: re-renders them when the language changes. */
export function useT(): typeof t {
  useI18n((s) => s.language);
  return t;
}

/**
 * A translated text with marked parts, e.g. "Find it in the <link>Ximilar App</link>.", each part rendered by
 * the function of its tag; translators may move the parts within the sentence.
 */
export function rich(text: string, parts: Record<string, (inner: string) => ReactNode>): ReactNode {
  const out: ReactNode[] = [];
  const re = /<(\w+)>(.*?)<\/\1>/g;
  let at = 0;
  for (let m = re.exec(text); m; m = re.exec(text)) {
    if (m.index > at) out.push(text.slice(at, m.index));
    const render = parts[m[1]];
    out.push(createElement(Fragment, { key: m.index }, render ? render(m[2]) : m[2]));
    at = m.index + m[0].length;
  }
  if (at < text.length) out.push(text.slice(at));
  return out;
}
