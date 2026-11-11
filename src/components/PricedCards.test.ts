import { describe, expect, it } from "vitest";
import { adjustPrice, parsePrice } from "./PricedCards";

describe("parsePrice", () => {
  it("reads thousands and decimal separators, and wants a positive amount", () => {
    expect(parsePrice("1,250")).toBe(1250);
    expect(parsePrice("1,250.00")).toBe(1250);
    expect(parsePrice("1.250,00")).toBe(1250);
    expect(parsePrice("12,5")).toBe(12.5);
    expect(parsePrice(" $24.99 ")).toBe(24.99);
    expect(parsePrice("9.999")).toBe(10);
    expect(parsePrice("0")).toBeNull();
    expect(parsePrice("-5")).toBeNull();
    expect(parsePrice("")).toBeNull();
    expect(parsePrice(undefined)).toBeNull();
    expect(parsePrice("12,")).toBeNull();
    expect(parsePrice("abc")).toBeNull();
  });
});

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
