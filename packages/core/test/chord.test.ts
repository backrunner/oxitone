import { describe, expect, it } from "vitest";
import { OxitoneError } from "@oxitone/protocol";
import { chord, Pattern, type ChordQuality } from "../src/index.js";

const INTERVALS: Record<ChordQuality, readonly number[]> = {
  major: [0, 4, 7],
  minor: [0, 3, 7],
  dim: [0, 3, 6],
  aug: [0, 4, 8],
  sus2: [0, 2, 7],
  sus4: [0, 5, 7],
};

function pitchesOf(pattern: Pattern): number[] {
  return pattern.notes.map((note) => note.pitch);
}

describe("chord", () => {
  it.each(Object.entries(INTERVALS))("builds a %s chord with correct intervals", (quality, intervals) => {
    const pattern = chord(60, quality as ChordQuality);
    expect(pitchesOf(pattern)).toEqual(intervals.map((i) => 60 + i));
  });

  it("applies inversions by rotating the bottom note up an octave", () => {
    expect(pitchesOf(chord(60, "major", { inversion: 1 }))).toEqual([64, 67, 72]);
    expect(pitchesOf(chord(60, "major", { inversion: 2 }))).toEqual([67, 72, 76]);
    expect(pitchesOf(chord(60, "major", { inversion: 3 }))).toEqual([72, 76, 79]);
  });

  it("supports open (drop-2) voicing", () => {
    // close: 60 64 67 -> drop the second-from-top (64) an octave: 52 60 67
    expect(pitchesOf(chord(60, "major", { voicing: "open" }))).toEqual([52, 60, 67]);
  });

  it("honors start, duration, velocity and lengthBeats", () => {
    const pattern = chord(48, "minor", { start: 2, duration: 1.5, velocity: 0.7 });
    expect(pattern.lengthBeats).toBe(3.5);
    for (const note of pattern.notes) {
      expect(note.start).toBe(2);
      expect(note.duration).toBe(1.5);
      expect(note.velocity).toBe(0.7);
    }
    expect(chord(48, "minor", { duration: 2, lengthBeats: 8 }).lengthBeats).toBe(8);
  });

  it("assigns voice hints in stacking order for stable sorting", () => {
    expect(chord(60, "major").notes.map((note) => note.voice)).toEqual([0, 1, 2]);
  });

  it("clamps pitches above 127", () => {
    expect(pitchesOf(chord(124, "aug"))).toEqual([124, 127, 127]);
  });

  it("rejects invalid roots, inversions and qualities", () => {
    expect(() => chord(128, "major")).toThrowError(OxitoneError);
    expect(() => chord(60, "major", { inversion: -1 })).toThrowError(OxitoneError);
    expect(() => chord(60, "major", { inversion: 1.5 })).toThrowError(OxitoneError);
    expect(() => chord(60, "mystery" as ChordQuality)).toThrowError(OxitoneError);
  });

  it("returns frozen patterns and never mutates inputs", () => {
    const options = { inversion: 1, velocity: 0.5 };
    const pattern = chord(60, "major", options);
    expect(options).toEqual({ inversion: 1, velocity: 0.5 });
    expect(Object.isFrozen(pattern.notes)).toBe(true);
    expect(pattern.notes.every((note) => Object.isFrozen(note))).toBe(true);
    expect(chord(60, "major")).not.toBe(chord(60, "major"));
  });
});
