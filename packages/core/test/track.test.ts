import { describe, expect, it } from "vitest";
import { ErrorCode } from "@oxitone/protocol";
import { Pattern, Project } from "../src/index.js";

describe("Track authoring settings", () => {
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
      expect(() => { track.midiChannel = value; }).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    }
    expect(() => { track.enabled = 0 as unknown as boolean; }).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
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
    track.add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }] })).at({ bar: 1 });
    track.midiChannel = 10;
    const report = await project.exportMidi({});
    expect(report.diagnostics.channelAssignments).toEqual([{ trackId: track.id, channel: 10, source: "explicit" }]);
    track.enabled = false;
    const disabled = await project.exportMidi({});
    expect(disabled.diagnostics.noteTrackCount).toBe(0);
    expect(disabled.diagnostics.channelAssignments).toEqual([]);
  });
});
