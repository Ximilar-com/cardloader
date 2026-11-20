import { describe, expect, it } from "vitest";
import type { Settings } from "./api";
import { answersFromSettings, answersToSettings, type SetupAnswers } from "./setup";

const base = {
  defaultTask: "identify",
  defaultIdMode: "auto",
  defaultSubcategory: "",
  defaultGrading: "off",
  defaultSources: ["folder"],
  priceStats: true,
  engine: "ximilar",
  setupDone: false,
} as Settings;

const answers = (a: Partial<SetupAnswers>): SetupAnswers => ({
  task: "identify",
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

  it("makes comics sessions for a comics collector", () => {
    const s = answersToSettings(answers({ interests: ["comics"], games: ["Pokemon"] }), base);
    expect(s.defaultIdMode).toBe("comics");
    expect(s.defaultSubcategory).toBe("");
    expect(answersFromSettings(s).interests).toEqual(["comics"]);
    expect(answersToSettings(answers({ interests: ["comics", "tcg"] }), base).defaultIdMode).toBe("auto");
    // Which kinds were mixed isn't stored.
    expect(answersFromSettings({ ...base, defaultIdMode: "auto" }).interests).toEqual(["tcg", "sport", "comics"]);
    expect(answersFromSettings({ ...base, defaultIdMode: "slab" }).interests).toEqual(["tcg", "sport", "comics"]);
  });

  it("grades when asked, else reads condition", () => {
    expect(answersToSettings(answers({ goals: ["grading", "condition"] }), base).defaultGrading).toBe("grade");
    expect(answersToSettings(answers({ goals: ["condition"] }), base).defaultGrading).toBe("condition");
    expect(answersToSettings(answers({}), base).defaultGrading).toBe("off");
  });

  it("keeps the identification defaults for cropping only", () => {
    const s = answersToSettings(answers({ task: "crop", interests: ["sport"], goals: ["prices"] }), base);
    expect(s.defaultTask).toBe("crop");
    expect(s.defaultIdMode).toBe("auto");
    expect(s.priceStats).toBe(true);
    expect(answersFromSettings(s).task).toBe("crop");
  });

  it("keeps the identification defaults when only grading or reading the condition", () => {
    for (const task of ["grade", "condition"] as const) {
      const s = answersToSettings(answers({ task, interests: ["sport"], engine: "local" }), base);
      expect(s.defaultTask).toBe(task);
      expect(s.engine).toBe("local");
      expect(s.defaultIdMode).toBe("auto");
      expect(s.defaultGrading).toBe("off");
    }
  });

  it("saves crops when asked, as part of identifying", () => {
    const s = answersToSettings(answers({ goals: ["grading", "crops"] }), base);
    expect(s.defaultSaveCrops).toBe(true);
    expect(s.defaultGrading).toBe("grade");
    expect(answersFromSettings(s).goals).toContain("crops");
    expect(answersToSettings(answers({}), base).defaultSaveCrops).toBe(false);
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
