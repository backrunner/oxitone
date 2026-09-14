import { describe, expect, it } from "vitest";
import { ErrorCode } from "@oxitone/protocol";
import { Pattern, Project } from "../src/index.js";

describe("Track authoring settings", () => {
  it("validates, serializes and clears independent tempo; content fitting uses local BPM", () => {
    const project = new Project();
    project.setTempo(240, "linear").addTempoSegment({ startBeat: 4, bpm: 999 });
    const track = project.addTrack();
    const revision = project.revision;
    track.tempo = 90;
    expect(project.revision).toBe(revision + 1);
    expect(project.snapshot().tracks[0]?.tempo).toBe(90);
    const sample = project.addSample({
      assetUri: "unread.wav",
      sha256: "00".repeat(32),
      format: "wav",
      sampleRate: 48000,
      channels: 1,
      frames: 96000,
    });
    expect(track.sample(sample).at({ bar: 2 }).fitToContent().durationBeats).toBe(3);
    const snapshot = project.snapshot();
    for (const bpm of [NaN, Infinity, 0, 19, 1000]) {
      expect(() => {
        track.tempo = bpm;
      }).toThrowError(expect.objectContaining({ code: ErrorCode.TempoRange }));
    }
    expect(project.snapshot()).toEqual(snapshot);
    track.tempo = undefined;
    expect(track.toSpec()).not.toHaveProperty("tempo");
  });
  it("validates MIDI channels and enablement before mutating revision", () => {
    const project = new Project();
    const track = project.addTrack();
    expect(track.enabled).toBe(true);
    expect(track.midiChannel).toBeUndefined();
    const before = project.revision;
    track.enabled = false;
    track.midiChannel = 10;
    expect(project.revision).toBe(before + 2);
    expect(track.toSpec()).toMatchObject({ enabled: false, midiChannel: 10 });
    const snapshot = project.snapshot();
    for (const value of [0, 17, 1.5, NaN, Infinity]) {
      expect(() => {
        track.midiChannel = value;
      }).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    }
    expect(() => {
      track.enabled = 0 as unknown as boolean;
    }).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    expect(project.snapshot()).toEqual(snapshot);
    track.enabled = true;
    track.midiChannel = undefined;
    expect(track.toSpec()).not.toHaveProperty("enabled");
    expect(track.toSpec()).not.toHaveProperty("midiChannel");
  });

  it("rejects channels from another project even when deterministic IDs match", () => {
    const first = new Project();
    const second = new Project();
    const local = first.addChannel();
    const foreign = second.addChannel();
    expect(local.id).toBe(foreign.id);
    const track = first.addTrack();
    const snapshot = first.snapshot();
    expect(() => track.use(foreign)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    expect(first.snapshot()).toEqual(snapshot);
    track.use(local).use(local);
    expect(track.channelIds).toEqual([local.id]);
  });

  it("honors explicit MIDI channels and disabled tracks in native SMF export", async () => {
    const project = new Project();
    const track = project.addTrack().use(project.addChannel());
    track
      .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }] }))
      .at({ bar: 1 });
    track.midiChannel = 10;
    const report = await project.exportMidi({});
    expect(report.diagnostics.channelAssignments).toEqual([{ trackId: track.id, channel: 10, source: "explicit" }]);
    track.enabled = false;
    const disabled = await project.exportMidi({});
    expect(disabled.diagnostics.noteTrackCount).toBe(0);
    expect(disabled.diagnostics.channelAssignments).toEqual([]);
  });

  it("exports the native independent clock as the equivalent project-beat arrangement", async () => {
    const make = (local: boolean) => {
      const project = new Project();
      project.setTempo(240);
      const track = project.addTrack().use(project.addChannel());
      if (local) track.tempo = 120;
      const length = local ? 1 : 2;
      track
        .add(
          new Pattern({
            id: "pat_clock",
            lengthBeats: length,
            notes: [{ pitch: 60, start: 0, duration: length, velocity: 1 }],
          }),
        )
        .at({ bar: 1, beat: length });
      return project;
    };
    const local = await make(true).exportMidi({});
    const expanded = await make(false).exportMidi({});
    expect(local.bytesBase64).toBe(expanded.bytesBase64);
  });
});
