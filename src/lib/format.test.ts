import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));

import { basename, defaultSessionName, duration, eta, grade, price } from "./format";

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

  it("names sessions by time", () => {
    expect(defaultSessionName(new Date(2026, 9, 15, 8, 5))).toBe("Session 2026-10-15 08:05");
  });
});
