import { describe, expect, it } from "vitest";
import { CSV_COLUMNS, csvColumnsOf, csvHeader, renamedTo, repeatedHeaders } from "./csvColumns";

describe("CSV columns", () => {
  it("reads a choice as export.rs does: each known column once, none = all", () => {
    expect(csvColumnsOf(["price", "name", "hologram", "price"])).toEqual(["price", "name"]);
    expect(csvColumnsOf([])).toEqual(CSV_COLUMNS);
    expect(csvColumnsOf(null)).toEqual(CSV_COLUMNS);
  });

  it("names a column as the file's header does", () => {
    expect(csvHeader("seq")).toBe("#");
    expect(csvHeader("listing_title")).toBe("listing_title");
  });
});

describe("renamed CSV headers", () => {
  it("writes the user's name for a column, else its own", () => {
    expect(csvHeader("name", { name: "  primary   name " })).toBe("primary name");
    expect(csvHeader("seq", { seq: " " })).toBe("#");
    expect(renamedTo({}, "name", "primary name")).toEqual({ name: "primary name" });
    expect(renamedTo({ name: "primary name" }, "name", "")).toEqual({});
    expect(renamedTo({ name: "primary name" }, "name", "name")).toEqual({});
    expect(repeatedHeaders(["name", "set_name"], { set_name: "name" })).toEqual(["name"]);
  });
});
