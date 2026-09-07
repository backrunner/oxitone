import { expect, it } from "vitest";
import { wavetable } from "@oxitone/core";
import { beatFromWire } from "@oxitone/protocol";
import { createLofiSong, lofi } from "../src/full/lofi.js";
import { createDubstepSong, dubstep } from "../src/full/melodic-dubstep.js";
import assets from "../src/full/piano-assets.json" with { type: "json" };
import { windowHook, horizonHook } from "../src/full/themes.js";

it.each([[lofi, createLofiSong], [dubstep, createDubstepSong]] as const)("builds a complete deterministic arrangement: %s", (song, create) => {
  // Model checks never download samples or require a local drum dylib.
  const snapshot = create(() => wavetable()).snapshot();
  expect(snapshot.patterns).toEqual(create(() => wavetable()).snapshot().patterns);
  expect(song.bars * 240 / song.bpm).toBeGreaterThan(175);
  expect(song.bars * 240 / song.bpm).toBeLessThan(185);
  expect(snapshot.tracks.length).toBeGreaterThanOrEqual(6);
  expect(snapshot.markers).toHaveLength(song.sections.length);
  const starts = snapshot.patternClips.map(clip => beatFromWire(clip.startBeat));
  expect(Math.max(...starts)).toBe((song.bars - 1) * 4);
  for (let i = 0; i < song.sections.length; i++) {
    const first = song.sections[i]![1] * 4, last = (song.sections[i + 1]?.[1] ?? song.bars) * 4;
    expect(starts.some(beat => beat >= first && beat < last)).toBe(true);
  }
  for (const pattern of snapshot.patterns) for (const note of pattern.notes) {
    expect(beatFromWire(note.start) + beatFromWire(note.duration)).toBeLessThanOrEqual(4);
  }
  expect(snapshot.mixerChannels.some(bus => bus.sends.length > 0)).toBe(true);
});

it("pins 13 key zones × 3 recorded dynamics to the upstream piano mapping", () => {
  expect(assets.files).toHaveLength(39);
  for (const file of assets.files) {
    const index = Number(file.file.match(/_(\d+)\.wav$/)![1]);
    expect(file.rootKey).toBe(21 + index * 2); // MappingChart entries 010…034.
    expect(file.sha256).toMatch(/^[a-f0-9]{64}$/);
  }
  for (const rootKey of new Set(assets.files.map(file => file.rootKey))) {
    expect(assets.files.filter(file => file.rootKey === rootKey).map(file => file.layer)).toEqual([1, 2, 3]);
  }
});

it.each([[windowHook, [0, 2, 4, 5, 7, 9, 10]], [horizonHook, [1, 2, 4, 6, 8, 9, 11]]] as const)(
  "keeps an eight-bar theme with repetition, answers and breathing space", (theme, scale) => {
    expect(theme(0)).toEqual(theme(2));
    expect(theme(8)).toEqual(theme(0));
    expect(theme(3)).not.toEqual(theme(0));
    expect(theme(7)).not.toEqual(theme(0));
    for (let bar = 0; bar < 8; bar++) {
      const notes = theme(bar);
      expect(notes.reduce((sum, n) => sum + n.duration, 0)).toBeLessThan(3.8);
      for (const n of notes) {
        expect(scale).toContain(n.pitch % 12);
        expect(n.start + n.duration).toBeLessThanOrEqual(4);
      }
    }
  });

it("develops the second drop and resolves each outro to its tonic", () => {
  const dub = createDubstepSong(() => wavetable()).snapshot();
  const answer = dub.tracks.find(t => t.name === "Drop II · answering phrase")!;
  const clips = dub.patternClips.filter(c => c.trackId === answer.id);
  expect(clips).toHaveLength(8);
  expect(clips.every(c => beatFromWire(c.startBeat) >= 72 * 4 && beatFromWire(c.startBeat) < 88 * 4)).toBe(true);
  expect(dub.patterns.some(p => p.name === "Sky · open final chorus")).toBe(true);
  for (const [snapshot, title, tonic] of [[dub, "F# · home", 6],
    [createLofiSong(() => wavetable()).snapshot(), "D · home", 2]] as const) {
    const cadence = snapshot.patterns.find(p => p.name === title)!;
    expect(cadence.notes).toHaveLength(1);
    expect(cadence.notes[0]!.pitch % 12).toBe(tonic);
    expect(beatFromWire(cadence.notes[0]!.duration)).toBeGreaterThanOrEqual(3);
  }
});
