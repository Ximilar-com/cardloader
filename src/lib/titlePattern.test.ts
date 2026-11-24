import { describe, expect, it } from "vitest";
import { fieldsIn, patternFromFields, patternOf, piecesOf, unknownParts, wordsOf } from "./titlePattern";

describe("title patterns", () => {
  it("splits a word into the user's text and parts", () => {
    expect(piecesOf("#{number}")).toEqual([{ text: "#" }, { field: "number", key: "number" }]);
    expect(piecesOf("({{Team}})")).toEqual([{ text: "(" }, { field: "team", key: "Team" }, { text: ")" }]);
    expect(piecesOf("{hologram}")).toEqual([{ field: null, key: "hologram" }]);
    expect(piecesOf("Auto")).toEqual([{ text: "Auto" }]);
    expect(piecesOf("{open")).toEqual([{ text: "{open" }]);
  });

  it("reads the parts a pattern names, and the names no part has", () => {
    const pattern = "  {name} {year}  Auto {printRun} {hologram} #{number} {name} ";
    expect(wordsOf(pattern)).toHaveLength(7);
    expect(fieldsIn(pattern)).toEqual(["name", "year", "printRun", "number"]);
    expect(unknownParts(pattern)).toEqual(["hologram"]);
  });

  it("writes an older template's parts as a pattern", () => {
    expect(patternFromFields(["year", "name", "number", "name"])).toBe("{year} {name} #{number}");
    expect(patternOf({ fields: ["name"], pattern: "" })).toBe("{name}");
    expect(patternOf({ fields: ["name"], pattern: "{name} RC" })).toBe("{name} RC");
  });
});
