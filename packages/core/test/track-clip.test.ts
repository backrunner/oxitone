import { describe, expect, it } from "vitest";
import { beatFromWire, OxitoneError } from "@oxitone/protocol";
import { Pattern, Project } from "../src/index.js";

function drums(): Pattern {
  return new Pattern({
    lengthBeats: 4,
    notes: [
      { pitch: 36, start: 0, duration: 0.5, velocity: 1 },
      { pitch: 38, start: 1, duration: 0.5, velocity: 0.9 },
    ],
  });
}

describe("bar/beat to startBeat conversion", () => {
  it("converts within a single 4/4 signature", () => {
    const project = new Project();
    const clip = project.addTrack().pattern(drums()).at({ bar: 3, beat: 2 });
    expect(clip.startBeat).toBe(10);
  });

  it("defaults beat to 0", () => {
    const project = new Project();
    const clip = project.addTrack().pattern(drums()).at({ bar: 2 });
    expect(clip.startBeat).toBe(4);
  });

  it("accumulates across time signature changes", () => {
    const project = new Project();
    project.addTimeSignature({ startBar: 3, numerator: 3, denominator: 4 });
    // bars 1-2 are 4/4 (8 beats), then 3/4 onwards.
    expect(project.barBeatToBeats({ bar: 3, beat: 0 })).toBe(8);
    expect(project.barBeatToBeats({ bar: 4, beat: 2 })).toBe(13);
    project.addTimeSignature({ startBar: 5, numerator: 6, denominator: 8 });
    // bars 3-4 are 3/4 (6 beats); bar 5 starts at 8 + 6 = 14 beats.
    expect(project.barBeatToBeats({ bar: 5, beat: 0 })).toBe(14);
    expect(project.barBeatToBeats({ bar: 6, beat: 1.5 })).toBe(14 + 3 + 1.5);
  });

  it("rejects beats outside the bar and bars below 1", () => {
    const project = new Project();
    expect(() => project.barBeatToBeats({ bar: 0 })).toThrowError(OxitoneError);
    expect(() => project.barBeatToBeats({ bar: 1, beat: 4 })).toThrowError(OxitoneError);
    expect(() => project.barBeatToBeats({ bar: 1.5 })).toThrowError(OxitoneError);
  });
});

describe("fluent clip API", () => {
  it("places clips through track.pattern(...).at(...)", () => {
    const project = new Project();
    const track = project.addTrack("keys");
    const clip = track.pattern(drums()).at({ bar: 1, beat: 0 }).loop(4);
    expect(clip.startBeat).toBe(0);
    expect(clip.loopCount).toBe(4);
    expect(clip.id).toMatch(/^pcl_/);
    expect(track.clips).toEqual([clip]);
    expect(track.toSpec().patternClipIds).toEqual([clip.id]);
  });

  it("supports track.add as an alias", () => {
    const project = new Project();
    const clip = project.addTrack().add(drums()).at({ bar: 1 });
    expect(clip.startBeat).toBe(0);
  });

  it("converts last({ bar }) with the project signatures", () => {
    const project = new Project();
    project.addTimeSignature({ startBar: 3, numerator: 3, denominator: 4 });
    const clip = project.addTrack().pattern(drums()).at({ bar: 1 }).last({ bar: 4 });
    expect(clip.lastBeat).toBe(11);
  });

  it("enforces loopCount/lastBeat mutual exclusion both ways", () => {
    const project = new Project();
    const withLoop = project.addTrack().pattern(drums()).at({ bar: 1 }).loop(2);
    expect(() => withLoop.last({ bar: 9 })).toThrowError(/mutually exclusive/);
    const withLast = project.addTrack().pattern(drums()).at({ bar: 1 }).last({ bar: 9 });
    expect(() => withLast.loop(2)).toThrowError(/mutually exclusive/);
  });

  it("validates loop count and lastBeat ordering", () => {
    const project = new Project();
    const track = project.addTrack();
    expect(() => track.pattern(drums()).at({ bar: 1 }).loop(0)).toThrowError(OxitoneError);
    expect(() => track.pattern(drums()).at({ bar: 1 }).loop(1.5)).toThrowError(OxitoneError);
    expect(() => track.pattern(drums()).at({ bar: 2 }).last({ bar: 1 })).toThrowError(OxitoneError);
  });

  it("validates transpose, velocityScale and probability", () => {
    const project = new Project();
    const clip = project.addTrack().pattern(drums()).at({ bar: 1 });
    clip.transpose(-12).velocityScale(1.5).probability(0.5).enabled(false);
    expect(clip.toSpec()).toMatchObject({
      transpose: -12,
      velocityScale: 1.5,
      probability: 0.5,
      enabled: false,
    });
    expect(() => clip.transpose(0.5)).toThrowError(OxitoneError);
    expect(() => clip.velocityScale(2.1)).toThrowError(OxitoneError);
    expect(() => clip.probability(-0.1)).toThrowError(OxitoneError);
  });

  it("omits default options from the wire spec", () => {
    const project = new Project();
    const clip = project.addTrack().pattern(drums()).at({ bar: 1 });
    const spec = clip.toSpec();
    expect(spec).not.toHaveProperty("transpose");
    expect(spec).not.toHaveProperty("velocityScale");
    expect(spec).not.toHaveProperty("probability");
    expect(spec).not.toHaveProperty("enabled");
    expect(beatFromWire(spec.startBeat)).toBe(0);
  });

  it("rejects placing the same draft twice", () => {
    const project = new Project();
    const draft = project.addTrack().pattern(drums());
    draft.at({ bar: 1 });
    expect(() => draft.at({ bar: 2 })).toThrowError(OxitoneError);
  });
});

describe("track channel binding", () => {
  it("binds multiple channels and dedupes", () => {
    const project = new Project();
    const track = project.addTrack();
    const a = project.addChannel({ name: "a" });
    const b = project.addChannel({ name: "b" });
    track.use(a).use(b).use(a);
    expect(track.channelIds).toEqual([a.id, b.id]);
  });

  it("keeps a clip owned by exactly one track", () => {
    const project = new Project();
    const pattern = drums();
    const first = project.addTrack("first");
    const second = project.addTrack("second");
    const clip = first.pattern(pattern).at({ bar: 1 });
    expect(clip.track).toBe(first);
    expect(first.clips).toHaveLength(1);
    expect(second.clips).toHaveLength(0);
    // The same immutable pattern may still be placed on another track as a
    // separate clip; the clip itself stays single-owner.
    const other = second.pattern(pattern).at({ bar: 5 });
    expect(other.track).toBe(second);
    expect(other.id).not.toBe(clip.id);
  });
});
