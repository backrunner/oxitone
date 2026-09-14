import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, describe, expect, it } from "vitest";
import { Pattern, Project } from "@oxitone/core";
import { ErrorCode, OxitoneError, PROTOCOL_VERSION, type ProjectSnapshot } from "@oxitone/protocol";
import { exportMidi } from "../src/index.js";

function buildProject(): Project {
  const project = new Project({ name: "midi-demo", seed: 11 });
  project.setTempo(120, "linear");
  project.addTempoSegment({ startBeat: 4, bpm: 180 });
  const channel = project.addChannel({ name: "keys" });
  const track = project.addTrack("keys").use(channel);
  track
    .pattern(
      new Pattern({
        id: "pat_riff",
        lengthBeats: 4,
        notes: [
          { pitch: 60, start: 0, duration: 1, velocity: 1 },
          { pitch: 64, start: 1, duration: 1, velocity: 0.5 },
        ],
      }),
    )
    .at({ bar: 1 })
    .loop(2);
  return project;
}

function rawSnapshot(trackCount: number): ProjectSnapshot {
  const beat = (numerator: number, denominator = 1) => ({ numerator, denominator });
  return {
    protocolVersion: PROTOCOL_VERSION,
    revision: "1",
    id: "prj_midi_e2e",
    sampleRate: 48_000,
    blockSize: 128,
    seed: 7,
    tempoMap: [{ startBeat: beat(0), bpm: 120 }],
    timeSignatureMap: [{ startBar: 1, numerator: 4, denominator: 4 }],
    markers: [],
    tracks: Array.from({ length: trackCount }, (_, i) => ({
      id: `trk_${String(i).padStart(2, "0")}`,
      channelIds: [],
      patternClipIds: [`clip_${String(i).padStart(2, "0")}`],
      sampleClipIds: [],
    })),
    patterns: [
      {
        id: "pat_a",
        lengthBeats: beat(4),
        notes: [{ pitch: 60, start: beat(0), duration: beat(1), velocity: 1 }],
      },
    ],
    patternClips: Array.from({ length: trackCount }, (_, i) => ({
      id: `clip_${String(i).padStart(2, "0")}`,
      patternId: "pat_a",
      trackId: `trk_${String(i).padStart(2, "0")}`,
      startBeat: beat(0),
    })),
    sampleClips: [],
    samples: [],
    channels: [],
    mixerChannels: [],
    automation: [],
  };
}

const workdirs: string[] = [];
afterAll(() => {
  for (const dir of workdirs) {
    rmSync(dir, { recursive: true, force: true });
  }
});

describe("exportMidi end-to-end through the native binding", () => {
  it("exports deterministic SMF Type 1 bytes from a Project", () => {
    const report = exportMidi(buildProject(), {});
    expect(report.bytesBase64).toBeDefined();
    const bytes = Buffer.from(report.bytesBase64 as string, "base64");
    expect(bytes.subarray(0, 4).toString("ascii")).toBe("MThd");
    expect(bytes.readUInt16BE(8)).toBe(1); // format 1
    expect(bytes.readUInt16BE(10)).toBe(2); // conductor + one note track
    expect(bytes.readUInt16BE(12)).toBe(960); // default PPQ
    expect(report.diagnostics.ppq).toBe(960);
    expect(report.diagnostics.channelAssignments).toHaveLength(1);
    expect(report.diagnostics.channelAssignments[0]).toMatchObject({
      channel: 1,
      source: "auto",
    });

    const again = exportMidi(buildProject(), {});
    expect(again.bytesBase64).toBe(report.bytesBase64);
  });

  it("resamples a linear tempo segment into discrete tempo events", () => {
    const report = exportMidi(buildProject(), {});
    // Same shape as the Rust golden: 32 resampled events + the step at beat 4.
    expect(report.diagnostics.tempoEventResolutionTicks).toBe(120);
    expect(report.diagnostics.tempoEventCount).toBe(33);
  });

  it("writes the file when options.path is set", () => {
    const dir = mkdtempSync(join(tmpdir(), "oxitone-midi-"));
    workdirs.push(dir);
    const path = join(dir, "export.mid");
    const report = exportMidi(buildProject(), { path, ppq: 480 });
    expect(report.path).toBe(path);
    expect(report.bytesBase64).toBeUndefined();
    const bytes = readFileSync(path);
    expect(bytes.length).toBe(report.bytes);
    expect(bytes.subarray(0, 4).toString("ascii")).toBe("MThd");
    expect(bytes.readUInt16BE(12)).toBe(480);
    expect(report.diagnostics.ppq).toBe(480);
  });

  it("accepts a raw ProjectSnapshot", () => {
    const report = exportMidi(rawSnapshot(2), {});
    expect(report.diagnostics.noteTrackCount).toBe(2);
    expect(report.diagnostics.channelAssignments.map((assignment) => assignment.channel)).toEqual([1, 2]);
  });

  it("fails with MidiChannelLimit and lists unassigned track IDs", () => {
    try {
      exportMidi(rawSnapshot(17), {});
      expect.unreachable();
    } catch (error) {
      expect(error).toBeInstanceOf(OxitoneError);
      const oxitoneError = error as OxitoneError;
      expect(oxitoneError.code).toBe(ErrorCode.MidiChannelLimit);
      const unassigned = oxitoneError.details?.unassignedTrackIds;
      expect(Array.isArray(unassigned)).toBe(true);
      expect(unassigned).toHaveLength(17);
      expect(unassigned).toContain("trk_00");
      expect(unassigned).toContain("trk_16");
    }
  });

  it("allows more than 16 tracks when every track shares explicit channels", () => {
    const snapshot = rawSnapshot(17);
    for (const track of snapshot.tracks) {
      track.midiChannel = 1;
    }
    const report = exportMidi(snapshot, {});
    expect(report.diagnostics.noteTrackCount).toBe(17);
    expect(
      report.diagnostics.channelAssignments.every(
        (assignment) => assignment.channel === 1 && assignment.source === "explicit",
      ),
    ).toBe(true);
  });
});
