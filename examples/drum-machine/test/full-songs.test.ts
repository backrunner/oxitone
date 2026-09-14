import { expect, it } from "vitest";
import { beatFromWire } from "@oxitone/protocol";
import { createDubstepSong, dubstep } from "../src/full/melodic-dubstep.js";
import { horizonHook } from "../src/full/themes.js";
import { midiSnapshot } from "../src/full/midi-export.js";

it("builds a deterministic sampled-piano and electronic arrangement with two developed drops", () => {
  const snapshot = createDubstepSong().snapshot();
  expect(snapshot).toEqual(createDubstepSong().snapshot());
  expect(snapshot.samples).toHaveLength(52);
  expect(
    snapshot.channels.every((c) =>
      ["oxitone.wavetable", "example.drums", "oxitone.multisampler"].includes(c.instrument.pluginId),
    ),
  ).toBe(true);
  expect((dubstep.bars * 240) / dubstep.bpm).toBeGreaterThan(175);
  expect((dubstep.bars * 240) / dubstep.bpm).toBeLessThan(185);
  expect(snapshot.tracks.length).toBeGreaterThanOrEqual(16);
  expect(snapshot.tracks.every((t) => t.midiChannel === undefined)).toBe(true);
  expect(snapshot.markers).toHaveLength(dubstep.sections.length);
  const starts = snapshot.patternClips.map((clip) => beatFromWire(clip.startBeat));
  expect(Math.max(...starts)).toBe((dubstep.bars - 1) * 4);
  for (let i = 0; i < dubstep.sections.length; i++) {
    const first = dubstep.sections[i]![1] * 4,
      last = (dubstep.sections[i + 1]?.[1] ?? dubstep.bars) * 4;
    expect(starts.some((beat) => beat >= first && beat < last)).toBe(true);
  }
  for (const pattern of snapshot.patterns)
    for (const note of pattern.notes) {
      expect(beatFromWire(note.start) + beatFromWire(note.duration)).toBeLessThanOrEqual(4);
    }
  const answer = snapshot.tracks.find((t) => t.name === "Drop II · answering phrase")!;
  const clips = snapshot.patternClips.filter((c) => c.trackId === answer.id);
  expect(clips).toHaveLength(8);
  expect(clips.every((c) => beatFromWire(c.startBeat) >= 72 * 4 && beatFromWire(c.startBeat) < 88 * 4)).toBe(true);
  expect(snapshot.patterns.some((p) => p.name === "Sky · open final chorus")).toBe(true);
  const cadence = snapshot.patterns.find((p) => p.name === "F# · home")!;
  expect(cadence.notes).toHaveLength(1);
  expect(cadence.notes[0]!.pitch % 12).toBe(6);
  expect(beatFromWire(cadence.notes[0]!.duration)).toBeGreaterThanOrEqual(3);
  expect(snapshot.mixerChannels.some((bus) => bus.sends.some((s) => s.sidechain))).toBe(true);
  const inserts = snapshot.channels
    .flatMap((c) => c.effectChain)
    .concat(snapshot.mixerChannels.flatMap((c) => c.inserts));
  for (const id of [
    "distortion",
    "multiband-dynamics",
    "delay",
    "reverb",
    "compressor",
    "limiter",
    "eq",
    "filter",
    "convolver",
    "spreader",
    "tape",
    "nonlinear-filter",
    "compactor",
  ]) {
    expect(inserts.some((f) => f.pluginId === `oxitone.${id}`)).toBe(true);
  }
  const sub = snapshot.channels.find((c) => c.name?.includes("pure mono sub"))!;
  expect(sub.instrument.parameters).toMatchObject({ "oscA.level": 0, "sub.wave": 0, "sub.octave": 0 });
  const bassline = snapshot.channels.find((c) => c.name?.includes("harmonic bassline"))!;
  expect(sub.mixerChannelId).not.toBe(bassline.mixerChannelId);
  const dropSub = snapshot.patterns.filter((p) => p.name === "Sub · sustained weight");
  expect(dropSub).toHaveLength(32);
  expect(dropSub.every((p) => beatFromWire(p.notes[0]!.duration) >= 3.12)).toBe(true);
});

it("preserves every lead note in both drops and develops the surrounding orchestration", () => {
  const s = createDubstepSong().snapshot();
  const clips = (name: string) => {
    const track = s.tracks.find((t) => t.name === name)!;
    return s.patternClips
      .filter((c) => c.trackId === track.id)
      .map((c) => ({
        bar: beatFromWire(c.startBeat) / 4,
        pattern: s.patterns.find((p) => p.id === c.patternId)!,
      }));
  };
  const leads = clips("Lead · horizon theme");
  expect(leads).toHaveLength(32);
  for (const { bar, pattern } of leads)
    expect(
      pattern.notes.map((n) => ({
        pitch: n.pitch,
        start: beatFromWire(n.start),
        duration: beatFromWire(n.duration),
        velocity: n.velocity,
      })),
    ).toEqual(horizonHook(bar));
  const halo = clips("Halo · final chorus air");
  expect(halo.map((c) => c.bar)).toEqual([80, 81, 82, 83, 84, 85, 86, 87]);
  const plucks = clips("Ember · chord pluck answers");
  expect(plucks.some((c) => c.bar >= 28 && c.bar < 40)).toBe(true);
  expect(plucks.some((c) => c.bar >= 24 && c.bar < 28)).toBe(false);
  const chords = clips("Supersaw chords");
  for (const bar of [31, 39, 79, 87]) {
    expect(chords.find((c) => c.bar === bar)!.pattern.notes.every((n) => beatFromWire(n.start) < 3)).toBe(true);
  }
  const kicks = clips("Kick");
  expect(kicks.some((c) => c.bar === 23 || c.bar === 71)).toBe(false);
  const bass = clips("Mid bass · syncopation"),
    vowel = clips("Vowel bass · response");
  for (const answer of vowel) {
    const starts = bass.find((c) => c.bar === answer.bar)!.pattern.notes.map((n) => beatFromWire(n.start));
    expect(answer.pattern.notes.every((n) => !starts.includes(beatFromWire(n.start)))).toBe(true);
  }
});

it("applies MIDI channel sharing only to a separate export snapshot", () => {
  const source = createDubstepSong().snapshot(),
    original = structuredClone(source);
  const midi = midiSnapshot(source);
  expect(source).toEqual(original);
  expect(midi.tracks.every((t) => t.midiChannel !== undefined && t.midiChannel >= 1 && t.midiChannel <= 16)).toBe(true);
  expect(midi.tracks.map(({ midiChannel: _midi, ...track }) => track)).toEqual(source.tracks);
});

it("keeps a repeated hook, answering phrases and breathing space in F# minor", () => {
  expect(horizonHook(0)).toEqual(horizonHook(2));
  expect(horizonHook(8)).toEqual(horizonHook(0));
  expect(horizonHook(3)).not.toEqual(horizonHook(0));
  expect(horizonHook(7)).not.toEqual(horizonHook(0));
  for (let bar = 0; bar < 8; bar++) {
    const notes = horizonHook(bar);
    expect(notes.reduce((sum, n) => sum + n.duration, 0)).toBeLessThan(3.8);
    for (const n of notes) {
      expect([1, 2, 4, 6, 8, 9, 11]).toContain(n.pitch % 12);
      expect(n.start + n.duration).toBeLessThanOrEqual(4);
    }
  }
});
