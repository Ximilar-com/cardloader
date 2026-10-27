import { describe, expect, it } from "vitest";
import { adjustPrice } from "./PricedCards";

describe("adjustPrice", () => {
  it("takes a share of the market price, ends in .99 and keeps a floor", () => {
    expect(adjustPrice(24.99, 100, false, 0)).toBe(24.99);
    expect(adjustPrice(24.99, 90, false, 0)).toBe(22.49);
    expect(adjustPrice(24.99, 90, true, 0)).toBe(22.99);
    expect(adjustPrice(10, 100, true, 0)).toBe(9.99);
    expect(adjustPrice(0.1, 80, true, 0.99)).toBe(0.99);
    expect(adjustPrice(0.3, 100, false, 0.5)).toBe(0.5);
  });
});
