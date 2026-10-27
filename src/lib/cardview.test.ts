import { describe, expect, it } from "vitest";
import {
  candidates,
  filterRows,
  matchOf,
  needsReview,
  representativePrice,
  sortRows,
  tier,
  type Row,
} from "./cardview";

describe("candidates", () => {
  it("normalizes alternatives with their distances", () => {
    const [a, b] = candidates(
      [
        {
          name: "Ob Nixilis of the Black Oath",
          full_name: "Ob Nixilis of the Black Oath Commander Masters (CMM) #517",
          set: "Commander Masters",
          set_code: "CMM",
          card_number: "517",
          year: 2023,
          links: { "tcgplayer.com": "https://example.com/517" },
          price_stats: [{ stats_type: "graded", value: { median: 9 } }],
        },
        { full_name: "Port Town Commander Masters (CMM) #1019" },
      ],
      [0.2, 0.31, 0.45],
    );
    expect(a).toMatchObject({
      name: "Ob Nixilis of the Black Oath",
      detail: "Commander Masters (CMM) · #517 · 2023",
      distance: 0.31,
      confidence: "medium",
      price: 9,
      index: 0,
    });
    expect(b).toMatchObject({
      index: 1,
      name: "Port Town Commander Masters (CMM) #1019",
      detail: "",
      confidence: "low",
      price: null,
    });
  });

  it("handles missing data", () => {
    expect(candidates(null)).toEqual([]);
    expect(candidates([{ name: "X" }])[0]).toMatchObject({ distance: null, confidence: null });
    // Indexes stay those of the response, so a pick addresses the right candidate.
    expect(candidates([null, { name: "Y" }]).map((c) => c.index)).toEqual([1]);
  });
});

describe("pricing and tiers", () => {
  it("prefers the ungraded median", () => {
    expect(
      representativePrice([
        { stats_type: "graded", value: { median: 50 } },
        { stats_type: "ungraded", value: { median: 5 } },
      ]),
    ).toBe(5);
    expect(representativePrice([{ stats_type: "overall", median: 3 }])).toBe(3);
    expect(representativePrice([])).toBeNull();
  });

  it("matches the Rust tiers", () => {
    expect([tier(0.3), tier(0.4), tier(0.41), tier(null)]).toEqual(["high", "medium", "low", null]);
  });
});

describe("matchOf", () => {
  const side = (name: string, distance: number) => ({
    _objects: [{ name: "Card", _identification: { best_match: { name }, distances: [distance] } }],
  });

  it("takes the closer side when the back was sent too (like the Rust side)", () => {
    expect(matchOf({ records: [side("Front", 0.4), side("Back", 0.2)] }).ident?.best_match.name).toBe("Back");
    expect(matchOf({ records: [side("Front", 0), side("Back", 0)] }).ident?.best_match.name).toBe("Front");
    const noBackMatch = { _objects: [{ name: "Card", _identification: { best_match: null } }] };
    expect(matchOf({ records: [side("Front", 0.4), noBackMatch] }).ident?.best_match.name).toBe("Front");
  });
});

describe("review", () => {
  const row = (seq: number, extra: Partial<Row>): Row => ({
    key: String(seq),
    seq,
    name: "",
    fullName: "",
    setName: "",
    cardNumber: "",
    idType: "tcg",
    confidence: null,
    price: null,
    grade: null,
    condition: null,
    ...extra,
  });

  it("asks to review every identified card until it is accepted", () => {
    expect(needsReview({ status: "done", reviewed: false })).toBe(true);
    expect(needsReview({ status: "grading", reviewed: false })).toBe(true);
    expect(needsReview({ status: "done", reviewed: true })).toBe(false);
    expect(needsReview({ status: "error", reviewed: false })).toBe(false);
    expect(needsReview({ status: "queued", reviewed: false })).toBe(false);
  });

  it("sorts with missing values last and filters by chip and text", () => {
    const rows = [
      row(1, { name: "Mew", price: 5, confidence: "low", toReview: true }),
      row(2, { name: "Abra", price: null, confidence: "high", location: "Box 2" }),
      row(3, { name: "Zubat", price: 20, confidence: "medium", status: "error" }),
    ];
    expect(sortRows(rows, { key: "price", desc: true }).map((r) => r.seq)).toEqual([3, 1, 2]);
    expect(sortRows(rows, { key: "price", desc: false }).map((r) => r.seq)).toEqual([1, 3, 2]);
    expect(sortRows(rows, { key: "name", desc: false }).map((r) => r.seq)).toEqual([2, 1, 3]);
    expect(sortRows(rows, { key: "confidence", desc: false }).map((r) => r.seq)).toEqual([2, 3, 1]);
    expect(filterRows(rows, "review").map((r) => r.seq)).toEqual([1]);
    expect(filterRows(rows, "failed").map((r) => r.seq)).toEqual([3]);
    expect(filterRows(rows, "all", "box 2").map((r) => r.seq)).toEqual([2]);
  });
});
