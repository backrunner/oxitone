import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { canonicalEncode, canonicalize } from "../src/base/canonical.js";

const fixtures = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "schemas", "fixtures");

function readFixture(rel: string): string {
  return readFileSync(join(fixtures, rel), "utf8");
}

describe("canonicalize", () => {
  it("sorts object keys", () => {
    expect(JSON.stringify(canonicalize({ b: 1, a: 2 }))).toBe('{"a":2,"b":1}');
  });

  it("sorts id-bearing arrays by stable id, keeps other arrays in order", () => {
    const value = {
      items: [{ id: "b_2" }, { id: "a_1" }],
      tempoMap: [{ startBeat: 8 }, { startBeat: 0 }],
    };
    const out = canonicalize(value) as { items: { id: string }[]; tempoMap: { startBeat: number }[] };
    expect(out.items.map((i) => i.id)).toEqual(["a_1", "b_2"]);
    expect(out.tempoMap.map((t) => t.startBeat)).toEqual([8, 0]);
  });

  it("rejects non-finite numbers and bigints", () => {
    expect(() => canonicalize(Number.NaN)).toThrow();
    expect(() => canonicalize(1n)).toThrow();
  });

  it("normalizes negative zero", () => {
    expect(JSON.stringify(canonicalize(-0))).toBe("0");
  });
});

describe("canonicalEncode fixtures", () => {
  it("byte-reproduces the project snapshot fixture", () => {
    const text = readFixture("project-snapshot.canonical.json");
    expect(canonicalEncode(JSON.parse(text))).toBe(text);
  });

  it.each(["gate", "wave", "chance", "curve", "nested"])("byte-reproduces the %s automation fixture", (name) => {
    const text = readFixture(`automation/${name}.canonical.json`);
    expect(canonicalEncode(JSON.parse(text))).toBe(text);
  });

  it("byte-reproduces the pcg32/hash64 vector files", () => {
    for (const rel of ["pcg32-v1.json", "hash64.json"]) {
      const text = readFixture(rel);
      expect(canonicalEncode(JSON.parse(text))).toBe(text);
    }
  });

  it("ends with a single trailing LF and uses 2-space indent", () => {
    const text = readFixture("project-snapshot.canonical.json");
    expect(text.endsWith("}\n")).toBe(true);
    expect(text).not.toContain("\r");
    expect(text).toContain('\n  "blockSize": 128,');
  });
});
