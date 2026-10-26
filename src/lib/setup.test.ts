import { describe, expect, it } from "vitest";
import type { Settings } from "./api";
import { answersFromSettings, answersToSettings, type SetupAnswers } from "./setup";

const base = {
  defaultIdMode: "auto",
  defaultSubcategory: "",
  defaultGrading: "off",
  defaultSources: ["folder"],
  priceStats: true,
  engine: "ximilar",
  setupDone: false,
} as Settings;

const answers = (a: Partial<SetupAnswers>): SetupAnswers => ({
  interests: ["tcg"],
  games: [],
  goals: [],
  sources: ["folder"],
  engine: "ximilar",
  ...a,
});

describe("setup", () => {
  it("makes TCG sessions with the one game chosen", () => {
    const s = answersToSettings(answers({ games: ["Pokemon"] }), base);
    expect(s.defaultIdMode).toBe("tcg");
    expect(s.defaultSubcategory).toBe("Pokemon");
    expect(s.setupDone).toBe(true);
    expect(answersToSettings(answers({ games: ["Pokemon", "Lorcana"] }), base).defaultSubcategory).toBe("");
  });

  it("auto-detects when both card types are scanned", () => {
    const s = answersToSettings(answers({ interests: ["tcg", "sport"], games: ["Pokemon"] }), base);
    expect(s.defaultIdMode).toBe("auto");
    expect(s.defaultSubcategory).toBe("");
    expect(answersToSettings(answers({ interests: ["sport"] }), base).defaultIdMode).toBe("sport");
  });

  it("grades when asked, else reads condition", () => {
    expect(answersToSettings(answers({ goals: ["grading", "condition"] }), base).defaultGrading).toBe("grade");
    expect(answersToSettings(answers({ goals: ["condition"] }), base).defaultGrading).toBe("condition");
    expect(answersToSettings(answers({}), base).defaultGrading).toBe("off");
  });

  it("asks for prices only when wanted", () => {
    expect(answersToSettings(answers({ goals: ["prices"] }), base).priceStats).toBe(true);
    expect(answersToSettings(answers({}), base).priceStats).toBe(false);
  });

  it("puts the scanner first among the ways of scanning", () => {
    expect(answersToSettings(answers({ sources: ["csv", "scanner", "folder"] }), base).defaultSources).toEqual([
      "scanner",
      "folder",
      "csv",
    ]);
    expect(answersToSettings(answers({ sources: [] }), base).defaultSources).toEqual(["folder"]);
  });

  it("reads the answers back from the settings", () => {
    const a = answers({ games: ["Pokemon"], goals: ["grading", "prices"], sources: ["scanner"], engine: "local" });
    expect(answersFromSettings(answersToSettings(a, base))).toEqual(a);
  });
});
