import { describe, expect, it } from "vitest";
import { ErrorCode } from "@oxitone/protocol";
import { createAutomationNamespace, Project } from "../src/index.js";

const value = (bpm: number) => Math.log(bpm / 20) / Math.log(999 / 20);

describe("Rust effective clock authoring queries", () => {
  it("converts seconds across a tempo step at a nonzero starting beat", () => {
    const project = new Project();
    project.addTempoSegment({ startBeat: 4, bpm: 240 });
    const revision = project.revision;
    expect(project.beatsForSeconds(3, 1)).toBeCloseTo(3, 8);
    expect(project.beatsForSeconds(4, 0)).toBe(0);
    expect(project.revision).toBe(revision);
    for (const seconds of [-1, NaN, Infinity]) expect(() => project.beatsForSeconds(0, seconds)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
  });

  it("fits through tempo lane loops and holds their final value", () => {
    const project = new Project();
    const source = createAutomationNamespace().curve([
      { beat: 0, value: value(120) }, { beat: 0.5, value: value(240) }, { beat: 1, value: value(60) },
    ], "step");
    project.addAutomationLane({ entityId: project.id, parameterId: "tempo" }, source, { loop: { lengthBeats: 1, count: 2 } });
    expect(project.beatsForSeconds(0, 0.75)).toBeCloseTo(2, 4);
    expect(project.beatsForSeconds(0, 1.75)).toBeCloseTo(3, 4);
    const sample = project.addSample({ assetUri: "unread.wav", sha256: "00".repeat(32), format: "wav", sampleRate: 48000, channels: 1, frames: 84000n });
    const clip = project.addTrack().sample(sample).at({ bar: 1 });
    expect(clip.fitToContent().durationBeats).toBeCloseTo(3, 4);
  });

  it("rejects excessive bake horizons before allocating the grid", () => {
    const project = new Project();
    const a = createAutomationNamespace();
    project.addAutomationLane({ entityId: project.id, parameterId: "tempo" }, a.constant(value(120)));
    expect(() => project.beatsForSeconds(0, 1e20)).toThrowError(expect.objectContaining({ code: ErrorCode.TempoMapComplexity }));
  });
});
