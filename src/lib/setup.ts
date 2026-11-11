// The first-run setup: what the user told us, and the defaults it becomes. Only the defaults are stored, so a
// rerun of the setup reads its answers back from them.

import type { Engine, GradingMode, IdMode, Settings, SourceKind } from "./api";

export type Interest = "tcg" | "sport" | "comics";
/** In the order the setup shows them. */
export const INTERESTS: Interest[] = ["tcg", "sport", "comics"];
export type Goal = "grading" | "condition" | "prices";

export interface SetupAnswers {
  interests: Interest[];
  /** TCG games; one of them is a default, several (or none) leave the game open. */
  games: string[];
  goals: Goal[];
  sources: SourceKind[];
  engine: Engine;
}

/** The order "New session" prefers: the first chosen source is the one it opens. */
export const SOURCE_ORDER: SourceKind[] = ["scanner", "folder", "watch_folder", "csv"];

export function answersToSettings<T extends Settings>(answers: SetupAnswers, settings: T): T {
  // One kind of collectible is every session's "Identify as"; several are told apart per card.
  const kinds = INTERESTS.filter((i) => answers.interests.includes(i));
  const idMode: IdMode = kinds.length === 1 ? kinds[0] : "auto";
  const grading: GradingMode = answers.goals.includes("grading")
    ? "grade"
    : answers.goals.includes("condition")
      ? "condition"
      : "off";
  const sources = SOURCE_ORDER.filter((s) => answers.sources.includes(s));
  return {
    ...settings,
    defaultIdMode: idMode,
    defaultSubcategory: idMode === "tcg" && answers.games.length === 1 ? answers.games[0] : "",
    defaultGrading: grading,
    priceStats: answers.goals.includes("prices"),
    defaultSources: sources.length ? sources : ["folder"],
    engine: answers.engine,
    setupDone: true,
  };
}

export function answersFromSettings(settings: Settings): SetupAnswers {
  // Which kinds "auto" stood for isn't stored: it reads back as all of them.
  const one = INTERESTS.find((i) => i === settings.defaultIdMode);
  const interests: Interest[] = one ? [one] : [...INTERESTS];
  const goals: Goal[] = [];
  if (settings.defaultGrading === "grade") goals.push("grading");
  if (settings.defaultGrading === "condition") goals.push("condition");
  if (settings.priceStats) goals.push("prices");
  return {
    interests,
    games: settings.defaultSubcategory ? [settings.defaultSubcategory] : [],
    goals,
    sources: settings.defaultSources.length ? settings.defaultSources : ["folder"],
    engine: settings.engine,
  };
}
