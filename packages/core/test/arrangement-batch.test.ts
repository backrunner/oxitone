import { expect, it } from "vitest";
import { Pattern, Project } from "../src/index.js";
const make = () => {
  const p = new Project();
  const a = p.addTrack("A"),
    b = p.addTrack("B");
  const ch = p.addChannel();
  a.use(ch);
  const pattern = new Pattern({ lengthBeats: 4, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.8 }] });
  a.pattern(pattern).at({ bar: 1 });
  a.pattern(pattern).at({ bar: 2 });
  return { p, a, b };
};
it("resolves all group addresses before cross-track moves reorder the flattened clips", () => {
  const { p, a, b } = make();
  const ids = a.clips.map((c) => c.id);
  p.arrange({
    action: "batch",
    edits: [0, 1].map((clip) => ({
      action: "move",
      kind: "pattern",
      resource: 0,
      clip,
      track: 1,
      startBeat: 8 + 4 * clip,
    })),
  });
  expect(a.clips).toHaveLength(0);
  expect(b.clips.map((c) => [c.id, c.startBeat])).toEqual([
    [ids[0], 8],
    [ids[1], 12],
  ]);
  p.arrange({
    action: "batch",
    edits: [0, 1].map((clip) => ({ action: "remove", kind: "pattern", resource: 0, clip })),
  });
  expect(b.clips).toHaveLength(0);
});
it("rejects a whole batch when a later operation is invalid, including its revision and future IDs", () => {
  const { p } = make();
  const before = p.snapshot();
  expect(() =>
    p.arrange({
      action: "batch",
      edits: [
        { action: "duplicate", kind: "pattern", resource: 0, clip: 0, track: 1, startBeat: 8 },
        { action: "resize", kind: "pattern", resource: 99, clip: 1, durationBeats: 8 },
      ],
    }),
  ).toThrow();
  expect(p.snapshot()).toEqual(before);
});
it("pastes a cut clip with its settings and relative lastBeat, without persisting execution IDs", () => {
  const { p, a, b } = make();
  const original = a.clips[0]!;
  original.transpose(7).velocityScale(0.6).enabled(false).last({ bar: 3 });
  const { id: _id, patternId: _pattern, trackId: _track, ...settings } = original.toSpec();
  p.arrange({ action: "remove", kind: "pattern", resource: 0, clip: 0 });
  p.arrange({ action: "paste", kind: "pattern", resource: 0, track: 1, startBeat: 12, settings, channels: [0] });
  expect(b.clips[0]?.toSpec()).toMatchObject({
    transpose: 7,
    velocityScale: 0.6,
    enabled: false,
    lastBeat: { numerator: 20, denominator: 1 },
  });
  expect(b.channelIds).toEqual(a.channelIds);
});
it("retains sample rate, loop, pan, gain and stretch settings across clipboard paste", () => {
  const p = new Project(),
    track = p.addTrack();
  const sample = p.addSample({
    assetUri: "loop.wav",
    sha256: "a".repeat(64),
    format: "wav",
    sampleRate: 48000,
    channels: 2,
    frames: 96000,
    musicalLengthBeats: 4,
  });
  const original = p.createSampleClip(
    track,
    sample,
    { bar: 1 },
    {
      gain: 0.6,
      pan: -0.25,
      rate: 1.5,
      tempoSync: "repitch",
      enabled: false,
      loop: { lengthBeats: 4, count: 2 },
      durationBeats: 8,
    },
  );
  const { id: _id, sampleId: _sample, trackId: _track, ...settings } = original.toSpec();
  p.arrange({ action: "remove", kind: "sample", resource: 0, clip: 0 });
  p.arrange({ action: "paste", kind: "sample", resource: 0, track: 0, startBeat: 12, settings });
  expect(track.sampleClips[0]?.toSpec()).toMatchObject({ ...settings, startBeat: { numerator: 12, denominator: 1 } });
});
