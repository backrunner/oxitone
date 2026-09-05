import { describe, expect, it } from "vitest";
import { beatFromWire, beatToWire, rationalFromF64 } from "../src/beat.js";
import { OxitoneError } from "../src/errors.js";

describe("beatToWire", () => {
  it("normalizes integers", () => {
    expect(beatToWire(0)).toEqual({ numerator: 0, denominator: 1 });
    expect(beatToWire(16)).toEqual({ numerator: 16, denominator: 1 });
  });

  it("reduces simple fractions", () => {
    expect(beatToWire(0.5)).toEqual({ numerator: 1, denominator: 2 });
    expect(beatToWire(0.25)).toEqual({ numerator: 1, denominator: 4 });
    expect(beatToWire(1.5)).toEqual({ numerator: 3, denominator: 2 });
  });

  it("approximates non-terminating decimals with small denominators", () => {
    expect(beatToWire(0.1)).toEqual({ numerator: 1, denominator: 10 });
    expect(beatToWire(1 / 3)).toEqual({ numerator: 1, denominator: 3 });
    expect(beatToWire(0.04)).toEqual({ numerator: 1, denominator: 25 });
  });

  it("round-trips through beatFromWire", () => {
    for (const value of [0, 0.5, 1 / 3, 3.75, 127.002]) {
      const wire = beatToWire(value);
      expect(Math.abs(beatFromWire(wire) - value)).toBeLessThan(1e-12);
    }
  });

  it("rejects negative, NaN and infinite beats", () => {
    expect(() => beatToWire(-1)).toThrow(OxitoneError);
    expect(() => beatToWire(Number.NaN)).toThrow(OxitoneError);
    expect(() => beatToWire(Number.POSITIVE_INFINITY)).toThrow(OxitoneError);
  });

  it("accepts and validates wire input", () => {
    expect(beatToWire({ numerator: 3, denominator: 8 })).toEqual({ numerator: 3, denominator: 8 });
    expect(() => beatToWire({ numerator: 2, denominator: 4 })).toThrow();
    expect(() => beatToWire({ numerator: 1, denominator: 0 })).toThrow();
  });
});

describe("rationalFromF64", () => {
  it("is exact for dyadic rationals", () => {
    expect(rationalFromF64(0.375)).toEqual({ numerator: 3n, denominator: 8n });
  });

  it("bounds the denominator", () => {
    const { denominator } = rationalFromF64(Math.PI - 3);
    expect(denominator).toBeLessThanOrEqual(0xffff_ffffn);
  });
});
