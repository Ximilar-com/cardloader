import { afterEach, describe, expect, it } from "vitest";
import { DICTIONARIES, languageOf, t, useI18n, type Language, type Plural } from ".";
import { LANGUAGES } from "./languages";

type Tree = { [key: string]: string | Plural | Tree };

/** Every text by dotted key; a plural's forms as `key#form`. */
function flatten(tree: Tree, prefix = "", out = new Map<string, string>()): Map<string, string> {
  for (const [k, v] of Object.entries(tree)) {
    const key = prefix + k;
    if (typeof v === "string") out.set(key, v);
    else if (typeof v.other === "string") for (const [form, text] of Object.entries(v)) out.set(`${key}#${form}`, text);
    else flatten(v as Tree, `${key}.`, out);
  }
  return out;
}

/** The `{placeholders}` and `<tags>` a text must keep, whatever the language. */
function marks(text: string): string[] {
  return [...text.matchAll(/\{\w+\}|<\/?\w+>/g)].map((m) => m[0]).sort();
}

const en = flatten(DICTIONARIES.en as Tree);
const keysOf = (m: Map<string, string>) => new Set([...m.keys()].map((k) => k.split("#")[0]));

afterEach(() => useI18n.getState().setLanguage("en"));

describe("dictionaries", () => {
  it("cover the languages offered", () => {
    expect(LANGUAGES.map((l) => l.code).sort()).toEqual(Object.keys(DICTIONARIES).sort());
  });

  for (const language of Object.keys(DICTIONARIES) as Language[]) {
    it(`${language} has every English text, none empty, with the same placeholders and tags`, () => {
      const dict = flatten(DICTIONARIES[language] as Tree);
      expect(keysOf(dict)).toEqual(keysOf(en));
      for (const [key, text] of dict) {
        expect(text.trim(), key).not.toBe("");
        const base = key.split("#")[0];
        const english = en.get(`${base}#other`) ?? en.get(base) ?? "";
        // A plural's singular may spell the number out ("one card"), so only `other` must keep every mark.
        if (!key.includes("#") || key.endsWith("#other")) expect(marks(text), key).toEqual(marks(english));
      }
    });
  }
});

describe("t", () => {
  it("fills placeholders and picks plural forms per language", () => {
    expect(t("common.cards", { count: 1 })).toBe("1 card");
    expect(t("common.cards", { count: 1200 })).toBe("1,200 cards");
    useI18n.getState().setLanguage("de");
    expect(t("common.cards", { count: 1200 })).toMatch(/^1\.200 /);
  });

  it("reads unknown languages as English", () => {
    expect(languageOf("xx")).toBe("en");
    expect(languageOf(undefined)).toBe("en");
    useI18n.getState().setLanguage("xx");
    expect(t("common.cancel")).toBe("Cancel");
  });
});
