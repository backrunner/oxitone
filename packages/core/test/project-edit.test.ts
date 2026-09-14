import { expect, it } from "vitest";
import { Project, Pattern, effect, createAutomationNamespace } from "../src/index.js";

it("hydrates Track M/S independently of shared Channels and rejects partial invalid edits", () => {
  const p = new Project(),
    c = p.addChannel(),
    a = p.addTrack().use(c),
    b = p.addTrack().use(c);
  p.configure({ kind: "track", index: 0, mute: true, solo: true });
  expect(a.toSpec()).toMatchObject({ mute: true, solo: true });
  expect(b.mute).toBe(false);
  expect(c.mute).toBe(false);
  expect(c.solo).toBe(false);
  const before = p.snapshot();
  expect(() => p.configure({ kind: "track", index: 0, mute: false, solo: null as unknown as boolean })).toThrow();
  expect(() => p.configure({ kind: "track", index: 0 })).toThrow();
  expect(p.snapshot()).toEqual(before);
  const restored = Project.fromSnapshot(before),
    track = restored.tracks[0]!;
  expect(track.mute).toBe(true);
  expect(track.solo).toBe(true);
  track.mute = false;
  track.solo = false;
  expect(track.toSpec().mute).toBeUndefined();
  expect(track.toSpec().solo).toBeUndefined();
});

it("validates mixer values before mutation and preserves untouched effect identities", () => {
  const p = new Project(),
    c = p.addChannel(),
    b = p.addMixerChannel();
  const bound = c.addEffect(effect("delay", { feedback: 0.3 }));
  const removed = c.addEffect(effect("delay"));
  bound.param("feedback").automate(createAutomationNamespace().constant(0.4));
  const before = p.snapshot();
  expect(() => p.configure({ kind: "channel", index: 0, values: { level: 0.4, pan: 2 } })).toThrow();
  expect(p.snapshot()).toEqual(before);
  p.configure({ kind: "channel", index: 0, values: { level: 0.6, pan: -0.4, mute: true, solo: true } });
  expect(c.toSpec()).toMatchObject({ level: 0.6, pan: -0.4, mute: true, solo: true });
  p.configure({ kind: "bus", index: p.mixerChannels.indexOf(b), values: { level: 0.8, pan: 0.3 } });
  expect(b.toSpec()).toMatchObject({ level: 0.8, balance: 0.3 });
  const configured = p.snapshot();
  expect(() =>
    p.configure({ kind: "effect", owner: "channel", index: 0, slot: 0, config: effect("reverb") }),
  ).toThrow();
  expect(p.snapshot()).toEqual(configured);
  p.configure({ kind: "effect", owner: "channel", index: 0, slot: 1, config: effect("reverb") });
  expect(c.effectInstances[0]!.id).toBe(bound.id);
  expect(c.effectInstances[1]!.id).not.toBe(removed.id);
  expect(p.snapshot().automation).toEqual(before.automation);
});

it("copies all placement options and moves exclusive ends without reordering same-track clips", () => {
  const p = new Project(),
    t = p.addTrack(),
    other = p.addTrack();
  const pattern = new Pattern({ lengthBeats: 4, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.8 }] });
  const clip = t
    .pattern(pattern)
    .at({ bar: 1 })
    .last({ bar: 3 })
    .transpose(12)
    .velocityScale(0.7)
    .probability(0.5)
    .enabled(false);
  const second = t.pattern(pattern).at({ bar: 4 });
  p.arrange({ action: "move", kind: "pattern", resource: 0, clip: 0, track: 0, startBeat: 4 });
  expect(t.clips.map((c) => c.id)).toEqual([clip.id, second.id]);
  expect(clip.lastBeat).toBe(12);
  p.arrange({ action: "duplicate", kind: "pattern", resource: 0, clip: 0, track: 1, startBeat: 20 });
  const copy = other.clips[0]!.toSpec();
  expect(copy).toEqual({
    ...clip.toSpec(),
    id: copy.id,
    trackId: other.id,
    startBeat: { numerator: 20, denominator: 1 },
    lastBeat: { numerator: 28, denominator: 1 },
  });
  expect(copy.id).not.toBe(clip.id);
  p.arrange({ action: "resize", kind: "pattern", resource: 0, clip: 0, durationBeats: 2 });
  expect(clip.lastBeat).toBe(6);
  p.arrange({ action: "enable", kind: "pattern", resource: 0, clip: 0, enabled: true });
  expect(clip.toSpec().enabled).not.toBe(false);
  expect(copy.enabled).toBe(false);
});

it("keeps explicit automation tempo and rejects changing it as a scalar", () => {
  const p = new Project();
  p.configure({ kind: "tempo", bpm: 135 });
  p.addAutomationLane({ entityId: p.id, parameterId: "tempo" }, createAutomationNamespace().constant(120));
  const before = p.snapshot();
  expect(() => p.configure({ kind: "tempo", bpm: 150 })).toThrow(/automated/);
  expect(p.snapshot()).toEqual(before);
});

it("copies sample settings and automation enable state, fits duration, and rejects mixed Track clocks", () => {
  const p = new Project(),
    t = p.addTrack(),
    destination = p.addTrack();
  const sample = p.addSample({
    assetUri: "sample.wav",
    sha256: "00".repeat(32),
    format: "wav",
    sampleRate: 48000,
    channels: 2,
    frames: 48000,
  });
  const clip = t
    .sample(sample)
    .at({ bar: 1 }, { durationBeats: 4, tempoSync: "repitch", gain: 0.6, pan: -0.2, rate: 1.25, enabled: false });
  const original = clip.toSpec();
  p.arrange({ action: "duplicate", kind: "sample", resource: 0, clip: 0, track: 1, startBeat: 8 });
  const copy = destination.sampleClips[0]!;
  expect(copy.toSpec()).toEqual({
    ...original,
    id: copy.id,
    trackId: destination.id,
    startBeat: { numerator: 8, denominator: 1 },
  });
  p.arrange({ action: "fitSample", kind: "sample", resource: 0, clip: 1, durationBeats: 2 });
  expect(copy.durationBeats).toBe(2);
  expect(copy.tempoSync).toBe("repitch");
  expect(clip.toSpec()).toEqual(original);
  t.tempo = 80;
  const before = p.snapshot();
  expect(() => p.arrange({ action: "move", kind: "sample", resource: 0, clip: 0, track: 1, startBeat: 4 })).toThrow(
    /both ends/,
  );
  expect(p.snapshot()).toEqual(before);
  const lane = p.addAutomationLane(
    { entityId: p.addChannel().id, parameterId: "level" },
    createAutomationNamespace().constant(0.5),
  );
  const automation = p.createAutomationClip(lane, destination, 0, 4);
  automation.enabled = false;
  p.arrange({ action: "duplicate", kind: "automation", resource: 0, clip: 0, track: 1, startBeat: 8 });
  p.arrange({ action: "resize", kind: "automation", resource: 0, clip: 1, durationBeats: 2 });
  expect(p.automationClips[1]!.toSpec()).toMatchObject({
    enabled: false,
    durationBeats: { numerator: 2, denominator: 1 },
  });
  expect(automation.durationBeats).toBe(4);
});
