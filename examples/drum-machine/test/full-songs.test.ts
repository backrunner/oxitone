import { expect, it } from "vitest";
import { wavetable } from "@oxitone/core";
import { beatFromWire } from "@oxitone/protocol";
import { createLofiSong, lofi } from "../src/full/lofi.js";
import { createDubstepSong, dubstep } from "../src/full/melodic-dubstep.js";
import assets from "../src/full/piano-assets.json" with { type: "json" };

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
