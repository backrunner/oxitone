import { describe, expect, it } from "vitest";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { Project } from "../src/index.js";

describe("Project construction", () => {
  it("applies defaults", () => {
    const project = new Project();
    expect(project.sampleRate).toBe(48_000);
    expect(project.blockSize).toBe(128);
    expect(project.seed).toBe(0);
    expect(project.revision).toBe(0);
    expect(project.id).toMatch(/^prj_/);
    expect(project.masterMixerChannelId).toMatch(/^mix_/);
  });

  it("accepts explicit options and generates deterministic ids per seed", () => {
    const a = new Project({ name: "a", sampleRate: 44_100, blockSize: 64, seed: 7 });
    const b = new Project({ name: "b", sampleRate: 44_100, blockSize: 64, seed: 7 });
    expect(a.sampleRate).toBe(44_100);
    expect(a.blockSize).toBe(64);
    expect(a.addTrack().id).toBe(b.addTrack().id);
  });

  it("rejects invalid engine numbers", () => {
    expect(() => new Project({ sampleRate: 0 })).toThrowError(OxitoneError);
    expect(() => new Project({ blockSize: 12.5 })).toThrowError(OxitoneError);
    expect(() => new Project({ seed: -1 })).toThrowError(OxitoneError);
  });
});

describe("tempo map", () => {
  it("defaults to 120 bpm at beat 0", () => {
    const project = new Project();
    expect(project.tempoMap).toEqual([{ startBeat: 0, bpm: 120 }]);
  });

  it("setTempo replaces the map and supports curves", () => {
    const project = new Project();
    project.setTempo(90);
    expect(project.tempoMap).toEqual([{ startBeat: 0, bpm: 90 }]);
    project.setTempo(140, "linear");
    expect(project.tempoMap).toEqual([{ startBeat: 0, bpm: 140, curve: "linear" }]);
  });

  it("appends segments with strictly increasing startBeat", () => {
    const project = new Project();
    project.addTempoSegment({ startBeat: 8, bpm: 60, curve: "exponential" });
    expect(project.tempoMap).toHaveLength(2);
    expect(() => project.addTempoSegment({ startBeat: 8, bpm: 70 })).toThrowError(OxitoneError);
    expect(() => project.addTempoSegment({ startBeat: 4, bpm: 70 })).toThrowError(OxitoneError);
  });

  it("rejects out-of-range bpm with TempoRange", () => {
    const project = new Project();
    for (const bpm of [0, 19.9, 999.1, Number.NaN, Number.POSITIVE_INFINITY]) {
      try {
        project.setTempo(bpm);
        expect.unreachable();
      } catch (error) {
        expect(OxitoneError.isOxitoneError(error)).toBe(true);
        expect((error as OxitoneError).code).toBe(ErrorCode.TempoRange);
      }
    }
    expect(() => project.addTempoSegment({ startBeat: 4, bpm: 10 })).toThrowError(OxitoneError);
  });
});

describe("time signature map", () => {
  it("defaults to 4/4 at bar 1", () => {
    const project = new Project();
    expect(project.timeSignatureMap).toEqual([{ startBar: 1, numerator: 4, denominator: 4 }]);
  });

  it("requires power-of-two denominators", () => {
    const project = new Project();
    expect(() => project.setTimeSignature(3, 3)).toThrowError(OxitoneError);
    expect(() => project.addTimeSignature({ startBar: 2, numerator: 6, denominator: 5 })).toThrowError(OxitoneError);
    project.setTimeSignature(6, 8);
    expect(project.timeSignatureMap[0]).toEqual({ startBar: 1, numerator: 6, denominator: 8 });
  });

  it("requires bars from 1 and strictly increasing", () => {
    const project = new Project();
    expect(() => project.addTimeSignature({ startBar: 0, numerator: 3, denominator: 4 })).toThrowError(OxitoneError);
    project.addTimeSignature({ startBar: 5, numerator: 3, denominator: 4 });
    expect(() => project.addTimeSignature({ startBar: 5, numerator: 7, denominator: 8 })).toThrowError(OxitoneError);
  });
});

describe("markers and revision", () => {
  it("stores named beat positions and rejects negative beats", () => {
    const project = new Project();
    const marker = project.addMarker("chorus", 16);
    expect(marker.id).toMatch(/^mrk_/);
    expect(project.markers).toEqual([marker]);
    expect(() => project.addMarker("bad", -1)).toThrowError(OxitoneError);
  });

  it("bumps revision monotonically on every mutation", () => {
    const project = new Project();
    expect(project.revision).toBe(0);
    project.setTempo(100);
    expect(project.revision).toBe(1);
    project.addTimeSignature({ startBar: 3, numerator: 3, denominator: 4 });
    expect(project.revision).toBe(2);
    project.addMarker("m", 4);
    expect(project.revision).toBe(3);
    const track = project.addTrack("t");
    expect(project.revision).toBe(4);
    const channel = project.addChannel();
    expect(project.revision).toBe(5);
    track.use(channel);
    expect(project.revision).toBe(6);
    track.use(channel);
    expect(project.revision).toBe(6);
  });
});
