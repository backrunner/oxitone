import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { beatToWire, canonicalEncode, ErrorCode, type ProjectSnapshot } from "@oxitone/protocol";
import { compile, createEngine, dispose, exportMidi, renderWav } from "@oxitone/native";
import { AutomationSource, IdGenerator, Pattern, Project } from "../src/index.js";

const rational = { numerator: 123456789, denominator: 4294967291 };
const fixture = (): ProjectSnapshot => JSON.parse(readFileSync(
  new URL("../../../schemas/fixtures/project-snapshot.canonical.json", import.meta.url), "utf8"));

describe("editable project restoration", () => {
  it("preserves exact rational values, note order/IDs, plugin state, sends and implicit Master", () => {
    const snapshot = fixture();
    snapshot.revision = "9007199254740993";
    snapshot.patterns[0]!.notes[0]!.start = rational;
    snapshot.patterns[0]!.notes[1]!.duration = rational;
    snapshot.patterns[0]!.lengthBeats = { numerator: 123456789, denominator: 9999991 };
    snapshot.patternClips[0]!.startBeat = rational;
    snapshot.patternClips[0]!.durationBeats = rational;
    snapshot.patternClips[0]!.enabled = true;
    snapshot.patternClips[0]!.transpose = 0;
    snapshot.patternClips[0]!.velocityScale = 1;
    snapshot.tracks[0]!.enabled = true;
    snapshot.tracks[0]!.tempo = 90;
    snapshot.tempoMap[1]!.startBeat = rational;
    snapshot.markers[0]!.startBeat = rational;
    delete snapshot.markers[0]!.name;
    snapshot.samples[0]!.musicalLengthBeats = rational;
    snapshot.sampleClips[0]!.startBeat = rational;
    snapshot.sampleClips[0]!.loop!.lengthBeats = rational;
    snapshot.automation[3]!.loop!.lengthBeats = rational;
    snapshot.automation[3]!.lastBeat = rational;
    snapshot.channels[1]!.instrument = { pluginId: "oxitone.slicer", pluginVersion: "1.0.0", parameters: {},
      state: { sampleId: snapshot.samples[0]!.id, slices: { grid: 4 }, playMode: "gate" } };
    const original = structuredClone(snapshot);
    const project = Project.fromSnapshot(snapshot);
    expect(canonicalEncode(project.snapshot())).toBe(canonicalEncode(original));
    expect(project.revisionBigInt).toBe(9007199254740993n);
    expect(() => project.revision).toThrowError(/revisionBigInt/);
    expect(project.tracks[0]!.clips[0]!.pattern.notes.map((note) => note.id)).toEqual(original.patterns[0]!.notes.map((note) => note.id));
    snapshot.patterns[0]!.notes.reverse();
    (snapshot.channels[1]!.instrument.state as { slices: { grid: number } }).slices.grid = 8;
    expect(project.snapshot()).toEqual(original);
    project.tracks[0]!.tempo = undefined;
    project.tracks[0]!.clips[0]!.transpose(12);
    project.tracks[0]!.clips[0]!.durationBeats = undefined;
    project.tracks[1]!.sampleClips[0]!.gain = 0.5;
    const edited = project.snapshot();
    expect(edited.revision).toBe("9007199254740997");
    expect(edited.tracks[0]!.tempo).toBeUndefined();
    expect(edited.patternClips[0]!.durationBeats).toBeUndefined();
    expect(edited.patternClips[0]!.startBeat).toEqual(rational);
    expect(edited.sampleClips[0]!.loop).toEqual(original.sampleClips[0]!.loop);
    expect(edited.patterns).toEqual(original.patterns);
    expect(edited.automation).toEqual(original.automation);
    expect(edited.mixerChannels).toEqual(original.mixerChannels);
    project.master.level = 0.6;
    expect(project.snapshot().mixerChannels.find((bus) => bus.id === "mix_master")?.level).toBe(0.6);
  });

  it("continues generating IDs after restoration and rejects cross-entity collisions", () => {
    const project = new Project({ id: "prj_ids", seed: 19 });
    const channel = project.addChannel();
    const track = project.addTrack().use(channel);
    track.add(new Pattern({ id: "pat_kept", lengthBeats: 1, notes: [{ id: "not_kept", pitch: 60, start: 0, duration: 1, velocity: 1 }] })).at({ bar: 1 });
    project.addMixerChannel();
    project.addMarker("m", 0);
    const restored = Project.fromSnapshot(project.snapshot());
    const previous = project.snapshot();
    const newChannel = restored.addChannel();
    expect(newChannel.id).not.toBe(channel.id);
    const newTrack = restored.addTrack().use(newChannel);
    newTrack.add(restored.tracks[0]!.clips[0]!.pattern).at({ bar: 2 });
    restored.addMixerChannel();
    restored.addMarker("new", 2);
    const revision = restored.revision;
    expect(() => newTrack.add(new Pattern({ id: "pat_bad", lengthBeats: 1, notes: [{ id: "not_kept", pitch: 61, start: 0, duration: 1, velocity: 1 }] })).at({ bar: 1 })).toThrowError(/registered/);
    expect(() => restored.addSample({ id: "pat_kept", assetUri: "unused.wav", sha256: "00".repeat(32), format: "wav", sampleRate: 48000, channels: 1, frames: 1 })).toThrowError(/duplicate/);
    expect(restored.revision).toBe(revision);
    expect(project.snapshot()).toEqual(previous);
    const generator = new IdGenerator(5);
    const first = new IdGenerator(5).next("pat_");
    generator.reserve(first);
    expect(generator.next("pat_")).not.toBe(first);
  });

  it("keeps maximum u64 revisions readable and rejects edits without mutation", () => {
    const project = new Project();
    const track = project.addTrack().use(project.addChannel());
    track.add(new Pattern({ lengthBeats: 1, notes: [] })).at({ bar: 1 });
    const snapshot = project.snapshot();
    snapshot.revision = "18446744073709551615";
    const restored = Project.fromSnapshot(snapshot);
    const operations = [() => restored.setTempo(80), () => restored.addChannel(), () => restored.addTrack(),
      () => { restored.master.level = 0.5; }, () => { restored.channels[0]!.pan = 0.2; },
      () => { restored.tracks[0]!.tempo = 90; }, () => restored.tracks[0]!.clips[0]!.transpose(12),
      () => { restored.tracks[0]!.clips[0]!.durationBeats = 2; }];
    for (const operation of operations) {
      expect(operation).toThrowError(/revision exhausted/);
      expect(restored.snapshot()).toEqual(snapshot);
    }
  });

  it("rejects malformed snapshots, duplicate IDs, inconsistent ownership and invalid edits", () => {
    expect(() => Project.fromSnapshot(null as unknown as ProjectSnapshot)).toThrowError(/protocolVersion/);
    const changes: ((snapshot: ProjectSnapshot) => void)[] = [
      (s) => { s.protocolVersion = "2.0"; },
      (s) => { s.tracks[0]!.id = s.tracks[1]!.id; },
      (s) => { s.patterns[0]!.notes[0]!.id = s.channels[0]!.id; },
      (s) => { s.tracks[0]!.patternClipIds = []; },
      (s) => { s.tracks[1]!.patternClipIds.push(s.patternClips[0]!.id); },
      (s) => { s.patternClips[0]!.patternId = "pat_missing"; },
      (s) => { s.samples[0]!.edits = { startFrame: s.samples[0]!.frames }; },
      (s) => { s.sampleClips[0]!.durationBeats = beatToWire(0); },
      (s) => { s.patternClips[0]!.durationBeats = beatToWire(0); },
      (s) => { s.channels[0]!.instrument.resources = { sample: "smp_missing" }; },
      (s) => { s.id = "mix_master"; },
    ];
    for (const change of changes) {
      const snapshot = fixture(); change(snapshot);
      expect(() => Project.fromSnapshot(snapshot)).toThrowError();
    }
    try { Project.fromSnapshot({ ...fixture(), protocolVersion: "2.0" }); }
    catch (error) { expect(error).toMatchObject({ code: ErrorCode.ProtocolVersionUnsupported }); }
  });

  it("preserves audible WAV and seeded MIDI output across restore and later edits", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-restore-"));
    const engine = createEngine();
    try {
      const source = new Project({ seed: 42 });
      const channel = source.addChannel();
      source.addTrack().use(channel).add(new Pattern({ lengthBeats: 1, notes: [
        { pitch: 60, start: 0, duration: 0.5, velocity: 0.8, chance: 0.8 },
        { pitch: 67, start: 0.5, duration: 0.5, velocity: 0.8, chance: 0.9 },
      ] })).at({ bar: 1 }).loop(2);
      const snapshot = source.snapshot();
      snapshot.patterns[0]!.notes.reverse();
      compile(engine, snapshot);
      const before = join(root, "before.wav");
      renderWav(engine, snapshot, { path: before, end: { seconds: 1 }, tailSeconds: 0 });
      const restored = Project.fromSnapshot(snapshot);
      const after = join(root, "after.wav");
      await restored.renderWav({ path: after, end: { seconds: 1 }, tailSeconds: 0 });
      expect(await readFile(after)).toEqual(await readFile(before));
      expect(await restored.exportMidi({})).toEqual(exportMidi(engine, snapshot, {}));
      restored.channels[0]!.mute = true;
      expect((await restored.renderWav({ path: after, end: { seconds: 1 }, tailSeconds: 0 })).files[0]!.peakDbfs).toBe(-144);
    } finally { dispose(engine); await rm(root, { recursive: true, force: true }); }
  });

  it("detaches automation source input from later caller mutations", () => {
    const input = { kind: "constant" as const, value: 0.5 };
    const source = new AutomationSource(input);
    input.value = 1;
    expect(source.toSpec()).toEqual({ kind: "constant", value: 0.5 });
  });
});
