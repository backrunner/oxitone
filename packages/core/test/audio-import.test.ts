import { expect, it } from "vitest";
import { Project } from "../src/index.js";
import type { ProjectAudioImport } from "@oxitone/protocol";

const edit: ProjectAudioImport = {
  name: "Frozen audio",
  startBeat: 4,
  sample: {
    assetUri: "assets/output.wav",
    sha256: "a".repeat(64),
    format: "wav",
    sampleRate: 48000,
    channels: 2,
    frames: "52800",
  },
};
it("adds only a frozen sample track with fixed playback rate and retains it across snapshot restore", () => {
  const project = new Project();
  project.setTempo(90);
  const before = project.snapshot();
  expect(project.importAudio(edit)).toBe(project);
  const snapshot = project.snapshot();
  expect(snapshot.channels).toEqual(before.channels);
  expect(snapshot.tracks).toHaveLength(1);
  expect(snapshot.samples[0]).toMatchObject(edit.sample);
  expect(snapshot.sampleClips[0]).toMatchObject({ startBeat: { numerator: 4, denominator: 1 }, tempoSync: "off" });
  expect(Project.fromSnapshot(snapshot).snapshot()).toEqual(snapshot);
  expect(snapshot.sampleClips[0]).not.toHaveProperty("durationBeats"); // Preserve the whole rendered tail.
});
it("rejects malformed resources/timing before allocating IDs or changing the project", () => {
  const project = new Project();
  const before = project.snapshot();
  for (const candidate of [
    { ...edit, startBeat: -1 },
    { ...edit, sample: { ...edit.sample, frames: "0" } },
    { ...edit, name: "" },
  ]) {
    expect(() => project.importAudio(candidate)).toThrow();
    expect(project.snapshot()).toEqual(before);
  }
  const fresh = new Project();
  project.importAudio(edit);
  fresh.importAudio(edit);
  expect(project.snapshot()).toEqual(fresh.snapshot());
});
it("reserves all three authoring revisions before mutation", () => {
  const snapshot = new Project().snapshot();
  snapshot.revision = (0xffff_ffff_ffff_ffffn - 2n).toString();
  const project = Project.fromSnapshot(snapshot);
  expect(() => project.importAudio(edit)).toThrow(/revision exhausted/);
  expect(project.snapshot()).toEqual(snapshot);
  snapshot.revision = (0xffff_ffff_ffff_ffffn - 3n).toString();
  const last = Project.fromSnapshot(snapshot);
  last.importAudio(edit);
  expect(last.revisionBigInt).toBe(0xffff_ffff_ffff_ffffn);
});
