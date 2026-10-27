// The languages the app speaks, each with its flag (SVGs: Windows doesn't draw flag emoji).

import BR from "country-flag-icons/react/3x2/BR";
import DE from "country-flag-icons/react/3x2/DE";
import ES from "country-flag-icons/react/3x2/ES";
import FR from "country-flag-icons/react/3x2/FR";
import IT from "country-flag-icons/react/3x2/IT";
import JP from "country-flag-icons/react/3x2/JP";
import KR from "country-flag-icons/react/3x2/KR";
import US from "country-flag-icons/react/3x2/US";
import type { Language } from ".";

export interface LanguageInfo {
  code: Language;
  /** In the language itself. */
  name: string;
  english: string;
  Flag: typeof US;
}

export const LANGUAGES: LanguageInfo[] = [
  { code: "en", name: "English", english: "English", Flag: US },
  { code: "ja", name: "日本語", english: "Japanese", Flag: JP },
  { code: "es", name: "Español", english: "Spanish", Flag: ES },
  { code: "ko", name: "한국어", english: "Korean", Flag: KR },
  { code: "fr", name: "Français", english: "French", Flag: FR },
  { code: "pt", name: "Português", english: "Portuguese", Flag: BR },
  { code: "de", name: "Deutsch", english: "German", Flag: DE },
  { code: "it", name: "Italiano", english: "Italian", Flag: IT },
];
