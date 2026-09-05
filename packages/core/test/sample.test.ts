import { describe, expect, it } from "vitest";
import { ErrorCode, type SampleEditSpec } from "@oxitone/protocol";
import { Pattern, Project } from "../src/index.js";

const sampleOptions = {
  assetUri: "assets/kick.wav",
  sha256: "00".repeat(32), format: "wav" as const,
  sampleRate: 48_000, channels: 2 as const, frames: 48_000, musicalLengthBeats: 4,
};

describe("Sample and SampleClip authoring", () => {
  it("serializes samples and clips into the project snapshot", () => {
    const project = new Project({ seed: 9 });
    const sample = project.addSample(sampleOptions);
    const track = project.addTrack("audio");
    const clip = track.sample(sample).at({ bar: 2, beat: 1 }, {
      tempoSync: "stretch", rate: 1.25, gain: 0.8, pan: -0.25, enabled: false,
    });
    clip.fitBeats(6);
    const snapshot = project.snapshot();
    expect(snapshot.samples).toEqual([sample.toSpec()]);
    expect(snapshot.sampleClips).toEqual([clip.toSpec()]);
    expect(snapshot.tracks[0]?.sampleClipIds).toEqual([clip.id]);
    expect(clip.startBeat).toBe(5);
    expect(clip.durationBeats).toBe(6);
    expect(clip.tempoSync).toBe("stretch");
  });

  it("fits fractional and cross-signature bars in beat space", () => {
    const project = new Project();
    const sample = project.addSample(sampleOptions);
    project.addTimeSignature({ startBar: 3, numerator: 3, denominator: 4 });
    const clip = project.addTrack().sample(sample).at({ bar: 3 });
    clip.fitBars(2);
    expect(clip.startBeat).toBe(8);
    expect(clip.durationBeats).toBe(6);
    clip.fitBars(0.5);
    expect(clip.durationBeats).toBe(1.5);
    clip.fitToContent();
    expect(clip.durationBeats).toBe(4);
  });

  it("keeps nested sample data immutable and validates edits/ranges", () => {
    const edits: SampleEditSpec = { startFrame: "100", endFrame: "1000", level: 0.5, tone: -0.25 };
    const project = new Project();
    const sample = project.addSample({ ...sampleOptions, edits });
    const original = project.snapshot();
    edits.level = 2;
    expect(project.snapshot()).toEqual(original);
    expect(sample.edits).toEqual({ startFrame: "100", endFrame: "1000", level: 0.5, tone: -0.25 });
    expect(() => project.addSample({ ...sampleOptions, frames: 0 })).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    expect(() => project.addSample({ ...sampleOptions, sha256: "bad" })).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    const clip = project.addTrack().sample(sample).at({ bar: 1 });
    expect(() => clip.fitBeats(0)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    expect(() => clip.fitBars(-1)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
  });

  it("supports loop descriptors and rejects mutually exclusive forms", () => {
    const project = new Project();
    const sample = project.addSample(sampleOptions);
    const clip = project.addTrack().sample(sample).at({ bar: 1 }, { loop: { startBeat: 1, lengthBeats: 2, count: 3 } });
    expect(clip.loop).toMatchObject({ startBeat: { numerator: 1, denominator: 1 }, count: 3 });
    expect(() => project.addTrack().sample(sample).at({ bar: 1 }, { loop: { lengthBeats: 2, count: 2, lastBeat: 8 } })).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
  });

  it("does not confuse sample clips with MIDI pattern clips", () => {
    const project = new Project();
    const sample = project.addSample(sampleOptions);
    const track = project.addTrack();
    track.sample(sample).at({ bar: 1 });
    track.add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }] })).at({ bar: 2 });
    expect(track.clips).toHaveLength(1);
    expect(track.sampleClips).toHaveLength(1);
    expect(project.snapshot().patternClips).toHaveLength(1);
    expect(project.snapshot().sampleClips).toHaveLength(1);
  });

  it("rejects attaching a sample owned by another project", () => {
    const first = new Project();
    const second = new Project();
    const sample = first.addSample(sampleOptions);
    expect(() => second.addTrack().sample(sample)).toThrowError(
      expect.objectContaining({ code: ErrorCode.InvalidProject }),
    );
  });
});
