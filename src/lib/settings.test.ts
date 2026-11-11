import { describe, expect, it } from "vitest";
import type { Settings } from "./api";
import { rebaseSettings } from "./settings";

const stored = {
  apiBaseUrl: "https://api.ximilar.com",
  defaultIdMode: "auto",
  defaultGrading: "off",
  defaultPairing: "single",
  defaultSources: ["folder"],
  language: "en",
  concurrency: 4,
  scan: { dpi: 300, color: "color", duplex: false, multifeed: "off" },
  ebay: { sandbox: false, appId: "", locationKey: "", username: null },
  mcp: { enabled: false, port: 47800, allowRun: false, allowSettings: false },
} as unknown as Settings;

const changed = (s: Settings, patch: Partial<Settings>): Settings => ({ ...s, ...patch });

describe("rebaseSettings", () => {
  it("takes what was saved elsewhere and keeps what the user changed", () => {
    const draft = changed(stored, { concurrency: 8 });
    // An agent changed the defaults, the New session dialog the scanner, since the draft was made.
    const current = changed(stored, {
      defaultIdMode: "tcg",
      defaultSources: ["scanner", "folder"],
      scan: { ...stored.scan, duplex: true },
    });
    expect(rebaseSettings(stored, draft, current)).toEqual({ ...current, concurrency: 8 });
  });

  it("merges nested settings field by field", () => {
    const draft = changed(stored, {
      scan: { ...stored.scan, dpi: 600 },
      mcp: { ...stored.mcp, allowRun: true },
    });
    const current = changed(stored, {
      scan: { ...stored.scan, duplex: true, multifeed: "default" },
      mcp: { ...stored.mcp, allowSettings: true },
      ebay: { ...stored.ebay, username: "demo-seller", locationKey: "home" },
    });
    expect(rebaseSettings(stored, draft, current)).toEqual({
      ...current,
      scan: { ...current.scan, dpi: 600 },
      mcp: { ...current.mcp, allowRun: true },
    });
  });

  it("lets the user's change win where both changed, and changes nothing without edits", () => {
    const draft = changed(stored, { language: "de", scan: { ...stored.scan, duplex: false, dpi: 400 } });
    const current = changed(stored, { language: "ja", scan: { ...stored.scan, dpi: 600 } });
    const out = rebaseSettings(stored, draft, current);
    expect(out.language).toBe("de");
    expect(out.scan.dpi).toBe(400);
    expect(rebaseSettings(stored, stored, current)).toEqual(current);
    expect(rebaseSettings(stored, draft, stored)).toEqual(draft);
  });

  it("keeps a list the user changed as a whole", () => {
    const draft = changed(stored, { defaultSources: ["csv"] });
    const current = changed(stored, { defaultSources: ["scanner", "folder"] });
    expect(rebaseSettings(stored, draft, current).defaultSources).toEqual(["csv"]);
  });
});
