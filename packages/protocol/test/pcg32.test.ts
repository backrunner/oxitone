import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { hash64, hash64Input, Pcg32, PCG32_INCREMENT, PCG32_MULTIPLIER } from "../src/pcg32.js";

const fixtures = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "schemas", "fixtures");

interface PcgVectorFile {
  algorithm: string;
  multiplier: string;
  increment: string;
  vectors: { seed: string; outputs: string[]; floats: string[] }[];
}

interface HashVectorFile {
  algorithm: string;
  vectors: { input: string; hash: string }[];
}

describe("pcg32-v1 golden vectors", () => {
  const file = JSON.parse(readFileSync(join(fixtures, "pcg32-v1.json"), "utf8")) as PcgVectorFile;

  it("pins the algorithm parameters", () => {
    expect(file.algorithm).toBe("pcg32-v1");
    expect(PCG32_MULTIPLIER.toString(10)).toBe(file.multiplier);
    expect(PCG32_INCREMENT.toString(10)).toBe(file.increment);
  });

  it("reproduces the recorded u32 streams", () => {
    for (const vector of file.vectors) {
      const rng = new Pcg32(BigInt(vector.seed));
      const outputs = vector.outputs.map(() => String(rng.nextU32()));
      expect(outputs).toEqual(vector.outputs);
      const floats = vector.floats.map(() => String(rng.nextFloat()));
      expect(floats).toEqual(vector.floats);
    }
  });

  it("is deterministic across instances", () => {
    const a = new Pcg32(42);
    const b = new Pcg32(42);
    for (let i = 0; i < 64; i += 1) {
      expect(a.nextU32()).toBe(b.nextU32());
    }
  });

  it("keeps floats inside [0, 1)", () => {
    const rng = new Pcg32(7);
    for (let i = 0; i < 1000; i += 1) {
      const f = rng.nextFloat();
      expect(f).toBeGreaterThanOrEqual(0);
      expect(f).toBeLessThan(1);
    }
  });
});

describe("hash64-v1 golden vectors", () => {
  const file = JSON.parse(readFileSync(join(fixtures, "hash64.json"), "utf8")) as HashVectorFile;

  it("reproduces the recorded hashes from encoded inputs", () => {
    for (const vector of file.vectors) {
      const parts = vector.input.split("|").map((part) => {
        const [tag, raw] = [part.slice(0, 2), part.slice(2)];
        return tag === "n:" ? BigInt(raw) : raw;
      });
      expect(hash64Input(...parts)).toBe(vector.input);
      expect(hash64(...parts).toString(10)).toBe(vector.hash);
    }
  });

  it("rejects out-of-range integers", () => {
    expect(() => hash64(-1)).toThrow(RangeError);
    expect(() => hash64(1n << 64n)).toThrow(RangeError);
  });
});
