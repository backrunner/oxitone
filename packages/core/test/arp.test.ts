import { describe, expect, it } from "vitest";
import { OxitoneError } from "@oxitone/protocol";
import { arp, type Pattern } from "../src/index.js";

function pitchesOf(pattern: Pattern): number[] {
  return pattern.notes.map((note) => note.pitch);
}

const C_MAJOR = [60, 64, 67];

describe("arp", () => {
  it("orders up and down by pitch", () => {
    expect(pitchesOf(arp(C_MAJOR, "up", 0.25))).toEqual([60, 64, 67]);
    expect(pitchesOf(arp(C_MAJOR, "down", 0.25))).toEqual([67, 64, 60]);
    expect(pitchesOf(arp([67, 60, 64], "up", 0.25))).toEqual([60, 64, 67]);
  });

  it("bounces upDown without repeating endpoints", () => {
    expect(pitchesOf(arp(C_MAJOR, "upDown", 0.25))).toEqual([60, 64, 67, 64]);
    expect(pitchesOf(arp([60, 67], "upDown", 0.25))).toEqual([60, 67]);
  });

  it("is deterministic for random order with the same seed", () => {
    const a = pitchesOf(arp(C_MAJOR, "random", 0.25, { seed: 42 }));
    const b = pitchesOf(arp(C_MAJOR, "random", 0.25, { seed: 42 }));
    expect(a).toEqual(b);
    expect([...a].sort((x, y) => x - y)).toEqual([60, 64, 67]);
    const c = pitchesOf(arp([60, 61, 62, 63, 64, 65, 66, 67], "random", 0.25, { seed: 1 }));
    const d = pitchesOf(arp([60, 61, 62, 63, 64, 65, 66, 67], "random", 0.25, { seed: 2 }));
    expect(c).not.toEqual(d);
  });

  it("spaces notes by rate and applies gate", () => {
    const pattern = arp(C_MAJOR, "up", 0.5, { gate: 0.5 });
    expect(pattern.notes.map((note) => note.start)).toEqual([0, 0.5, 1]);
    expect(pattern.notes.map((note) => note.duration)).toEqual([0.25, 0.25, 0.25]);
    expect(pattern.lengthBeats).toBe(1.5);
  });

  it("stacks octaves upward and clamps at 127", () => {
    expect(pitchesOf(arp(C_MAJOR, "up", 0.25, { octaves: 2 }))).toEqual([60, 64, 67, 72, 76, 79]);
    expect(pitchesOf(arp([126], "up", 0.25, { octaves: 2 }))).toEqual([126, 127]);
    expect(arp(C_MAJOR, "up", 0.25, { octaves: 2 }).lengthBeats).toBe(1.5);
  });

  it("applies a linear velocity curve scaled by base velocity", () => {
    const pattern = arp(C_MAJOR, "up", 0.25, { velocity: 0.8, velocityCurve: { from: 0, to: 1 } });
    expect(pattern.notes.map((note) => note.velocity)).toEqual([0, 0.4, 0.8]);
    const flat = arp(C_MAJOR, "up", 0.25, { velocity: 0.6 });
    expect(flat.notes.map((note) => note.velocity)).toEqual([0.6, 0.6, 0.6]);
  });

  it("accepts note inputs and preserves determinism of voice order", () => {
    const pattern = arp(
      [
        { pitch: 67, start: 0, duration: 1, velocity: 1 },
        { pitch: 60, start: 0, duration: 1, velocity: 1 },
      ],
      "up",
      1,
    );
    expect(pitchesOf(pattern)).toEqual([60, 67]);
    expect(pattern.notes.map((note) => note.voice)).toEqual([0, 1]);
  });

  it("rejects invalid inputs", () => {
    expect(() => arp([], "up", 0.25)).toThrowError(OxitoneError);
    expect(() => arp(C_MAJOR, "up", 0)).toThrowError(OxitoneError);
    expect(() => arp(C_MAJOR, "up", 0.25, { gate: 0 })).toThrowError(OxitoneError);
    expect(() => arp(C_MAJOR, "up", 0.25, { gate: 1.5 })).toThrowError(OxitoneError);
    expect(() => arp(C_MAJOR, "up", 0.25, { octaves: 0 })).toThrowError(OxitoneError);
    expect(() => arp([128], "up", 0.25)).toThrowError(OxitoneError);
    expect(() => arp(C_MAJOR, "up", 0.25, { velocityCurve: { from: -0.1, to: 1 } })).toThrowError(
      OxitoneError,
    );
  });

  it("does not mutate the input array", () => {
    const input = [67, 60, 64];
    arp(input, "down", 0.25);
    arp(input, "random", 0.25, { seed: 9 });
    expect(input).toEqual([67, 60, 64]);
  });
});
