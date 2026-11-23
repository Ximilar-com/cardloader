import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));

import type { Engine, GradingMode, Prices, SessionOptions, Stage } from "./api";
import {
  agentOf,
  basename,
  cropEngineOf,
  cropsOf,
  defaultSessionName,
  duration,
  engineOf,
  eta,
  grade,
  gradeEngineOf,
  gradingOf,
  parseAmount,
  price,
  pricesOf,
  stagesOf,
  tasksOf,
  usesApi,
  withGradeMode,
  withTasks,
  type TaskChoice,
} from "./format";
import cases from "./session-options.cases.json";

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

  // What a session's options mean is written twice, here and in Rust (`SessionOptions` in `model.rs`). Both are
  // checked against the same cases, so neither drifts from the other.
  it("reads a session's options as the shared cases say", () => {
    expect(cases.length).toBeGreaterThan(10);
    for (const c of cases) {
      const o = c.options as SessionOptions;
      const what = JSON.stringify(c.options);
      expect(gradingOf(o), what).toBe(c.gradingMode as GradingMode);
      expect(usesApi(o), what).toBe(c.usesApi);
      expect(agentOf(o), what).toBe(c.agent);
      expect(cropsOf(o), what).toBe(c.crops);
      // Only identifying can be an agent's.
      expect(engineOf(o) === "agent", what).toBe(c.agent);
      expect(gradeEngineOf(o), what).toBe(c.gradeEngine as Engine);
      expect(cropEngineOf(o), what).toBe(c.cropEngine as Engine);
      expect(pricesOf(o), what).toBe(c.prices as Prices);
      expect(stagesOf(o), what).toEqual(c.stages as Stage[]);
    }
  });

  it("maps the chosen tasks onto a session's options and back", () => {
    const base = {
      ...cases[0].options,
      task: "identify",
      engine: "ximilar",
      grading: "off",
      saveCrops: false,
      priceStats: false,
      prices: "off",
    } as SessionOptions;
    const roundTrip = (tasks: TaskChoice[]) => tasksOf(withTasks(base, tasks));
    for (const tasks of [
      ["identify"],
      ["identify", "prices"],
      ["identify", "prices", "grade", "crop"],
      ["grade"],
      ["grade", "crop"],
      ["crop"],
    ] as TaskChoice[][]) {
      expect(roundTrip(tasks), tasks.join("+")).toEqual(tasks);
    }
    // Prices need identification.
    expect(roundTrip(["prices", "grade"])).toEqual(["grade"]);
    // Ximilar's prices when Ximilar identifies, else eBay's.
    expect(withTasks(base, ["identify", "prices"]).prices).toBe("ximilar");
    expect(withTasks({ ...base, engine: "local" }, ["identify", "prices"]).prices).toBe("ebay");

    // The condition is a grading mode: without identification it is a condition session, and back.
    const condition = withGradeMode(withTasks(base, ["identify", "grade"]), "condition");
    expect(condition.grading).toBe("condition");
    const only = withTasks(condition, ["grade"]);
    expect(gradingOf(only)).toBe("condition");
    expect(only.task).toBe("condition");
    expect(withTasks(only, ["identify", "grade"]).grading).toBe("condition");
    expect(withGradeMode(withTasks(base, ["grade"]), "centering")).toMatchObject({
      task: "grade",
      grading: "centering",
    });
  });
});
