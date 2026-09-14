import { describe, expect, it } from "vitest";
import {
  beatFromWire,
  decodeProjectSnapshot,
  encodeProjectSnapshot,
  OxitoneError,
  projectSnapshotSchema,
  PROTOCOL_VERSION,
} from "@oxitone/protocol";
import { arp, chord, Pattern, Project } from "../src/index.js";

function buildProject(): Project {
  const project = new Project({ name: "demo", seed: 11 });
  project.setTempo(128);
  project.addTempoSegment({ startBeat: 16, bpm: 90, curve: "linear" });
  project.addTimeSignature({ startBar: 5, numerator: 3, denominator: 4 });
  project.addMarker("drop", 16);
  const channel = project.addChannel({ name: "keys", level: 0.8, pan: -0.25 });
  const track = project.addTrack("keys").use(channel);
  const riff = new Pattern({
    id: "pat_riff",
    name: "riff",
    lengthBeats: 4,
    notes: [
      { pitch: 60, start: 0.5, duration: 0.5, velocity: 0.9, tags: ["melody"] },
      { pitch: 64, start: 0, duration: 0.25, velocity: 0.8, voice: 1 },
      { pitch: 67, start: 0, duration: 0.25, velocity: 0.7 },
    ],
  });
  track.pattern(riff).at({ bar: 1, beat: 0 }).loop(2).transpose(12);
  track
    .pattern(chord(60, "minor", { duration: 4, id: "pat_chord" }))
    .at({ bar: 3 })
    .last({ bar: 6 });
  track.pattern(arp([60, 64, 67], "up", 0.25, { seed: 3, id: "pat_arp" })).at({ bar: 9 });
  return project;
}

describe("snapshot", () => {
  it("passes the protocol schema", () => {
    const snapshot = buildProject().snapshot();
    expect(() => projectSnapshotSchema.parse(snapshot)).not.toThrow();
    expect(snapshot.protocolVersion).toBe(PROTOCOL_VERSION);
  });

  it("round-trips through canonical encode/decode", () => {
    const snapshot = buildProject().snapshot();
    const decoded = decodeProjectSnapshot(encodeProjectSnapshot(snapshot));
    expect(decoded).toEqual(snapshot);
  });

  it("restores a snapshot into editable builders without changing wire state", () => {
    const original = buildProject();
    const snapshot = original.snapshot();
    const restored = Project.fromSnapshot(snapshot);
    expect(restored.snapshot()).toEqual(snapshot);
    expect(restored.tracks[0]?.clips[0]?.pattern.id).toBe("pat_riff");
    restored.setTempo(100);
    expect(restored.revision).toBe(Number(snapshot.revision) + 1);
    const added = restored.addTrack("new");
    expect([restored.id, ...restored.tracks.map((track) => track.id)]).toContain(added.id);
    expect(new Set(restored.tracks.map((track) => track.id)).size).toBe(restored.tracks.length);
  });

  it("normalizes beats to reduced wire rationals", () => {
    const snapshot = buildProject().snapshot();
    const riff = snapshot.patterns.find((pattern) => pattern.name === "riff");
    expect(riff?.lengthBeats).toEqual({ numerator: 4, denominator: 1 });
    const starts = riff?.notes.map((note) => note.start);
    expect(starts).toContainEqual({ numerator: 1, denominator: 2 });
    const clip = snapshot.patternClips[0];
    expect(clip?.startBeat).toEqual({ numerator: 0, denominator: 1 });
    // bars 1-4 in 4/4 (16 beats), bar 5 in 3/4 -> bar 6 starts at beat 19.
    expect(beatFromWire(snapshot.patternClips[1]?.lastBeat ?? { numerator: 0, denominator: 1 })).toBe(19);
  });

  it("sorts simultaneous notes by start, voice, creation order", () => {
    const snapshot = buildProject().snapshot();
    const riff = snapshot.patterns.find((pattern) => pattern.name === "riff");
    expect(riff?.notes.map((note) => note.pitch)).toEqual([67, 64, 60]);
  });

  it("emits the master mixer channel and channel routing", () => {
    const project = buildProject();
    const snapshot = project.snapshot();
    expect(snapshot.mixerChannels).toHaveLength(1);
    expect(snapshot.mixerChannels[0]).toMatchObject({ id: project.masterMixerChannelId, name: "Master" });
    expect(snapshot.channels[0]?.mixerChannelId).toBe(project.masterMixerChannelId);
    expect(snapshot.channels[0]?.instrument.pluginId).toBe("oxitone.wavetable");
  });

  it("records revision as a monotonically increasing wire counter", () => {
    const project = new Project();
    const r0 = project.snapshot().revision;
    project.setTempo(100);
    const r1 = project.snapshot().revision;
    project.addTrack();
    const r2 = project.snapshot().revision;
    expect(BigInt(r1)).toBeGreaterThan(BigInt(r0));
    expect(BigInt(r2)).toBeGreaterThan(BigInt(r1));
    expect(r2).toBe("2");
  });

  it("is stable for identical seeded projects", () => {
    expect(buildProject().snapshot()).toEqual(buildProject().snapshot());
  });

  it("rejects duplicate pattern ids from different objects", () => {
    const project = new Project();
    const track = project.addTrack();
    const a = new Pattern({ id: "pat_dup", lengthBeats: 1, notes: [] });
    const b = new Pattern({ id: "pat_dup", lengthBeats: 2, notes: [] });
    track.pattern(a).at({ bar: 1 });
    expect(() => track.pattern(b).at({ bar: 2 })).toThrowError(OxitoneError);
  });
});
