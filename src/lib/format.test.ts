import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));

import { basename, defaultSessionName, duration, eta, grade, parseAmount, price } from "./format";

describe("format", () => {
  it("formats prices and grades", () => {
    expect(price(24.99)).toBe("$24.99");
    expect(price(null)).toBe("—");
    expect(grade(9)).toBe("9");
    expect(grade(7.5)).toBe("7.5");
  });

  it("takes basenames of both path styles", () => {
    expect(basename("/Users/me/cards/a.jpg")).toBe("a.jpg");
    expect(basename("C:\\scans\\b.png")).toBe("b.png");
  });

  it("formats durations", () => {
    expect(duration(42)).toBe("42s");
    expect(duration(200)).toBe("3m 20s");
    expect(duration(3900)).toBe("1h 5m");
  });

  it("estimates time left from the progress rate", () => {
    expect(
      eta(
        [
          [0, 0],
          [10_000, 5],
        ],
        10,
      ),
    ).toBe(20);
    expect(eta([[0, 0]], 10)).toBeNull();
    expect(
      eta(
        [
          [0, 3],
          [5_000, 3],
        ],
        10,
      ),
    ).toBeNull();
  });

  it("reads amounts typed with thousands and decimal separators", () => {
    // Both: the last one is the decimal separator.
    expect(parseAmount("1,250.00")).toBe(1250);
    expect(parseAmount("1.250,00")).toBe(1250);
    expect(parseAmount("1,234,567.89")).toBe(1234567.89);
    expect(parseAmount("1.234.567,89")).toBe(1234567.89);
    // Commas only: three digits after each separate thousands, one or two after a lone comma are decimals.
    expect(parseAmount("1,250")).toBe(1250);
    expect(parseAmount("1,250,000")).toBe(1250000);
    expect(parseAmount("12,5")).toBe(12.5);
    expect(parseAmount("12,50")).toBe(12.5);
    expect(parseAmount("12.50")).toBe(12.5);
    expect(parseAmount("1.250")).toBe(1.25);
    expect(parseAmount("7")).toBe(7);
    expect(parseAmount("-3,5")).toBe(-3.5);
    // "$" and spaces around.
    expect(parseAmount(" $1,250 ")).toBe(1250);
    expect(parseAmount("$ 0.99")).toBe(0.99);
  });

  it("refuses what isn't an amount", () => {
    for (const text of [
      "",
      "  ",
      "$",
      "abc",
      "12,",
      ",5",
      "1,2345",
      "1,250,5",
      "12,50,000",
      "1.2.3",
      "1e3",
      "Infinity",
      "12 $",
      "1 250",
    ]) {
      expect(parseAmount(text), text).toBeNull();
    }
  });

  it("names sessions by time", () => {
    expect(defaultSessionName(new Date(2026, 9, 15, 8, 5))).toBe("Session 2026-10-15 08:05");
  });
});
