import { expect, it } from "vitest";
import { Pattern, Project } from "../src/index.js";

function fixture() {
  const project = new Project();
  const from = project.addTrack().use(project.addChannel());
  const to = project.addTrack();
  const pattern = from
    .add(new Pattern({ lengthBeats: 4, notes: [] }))
    .at({ bar: 1 })
    .last({ bar: 3 });
  const sample = from
    .sample(
      project.addSample({
        assetUri: "sample.wav",
        sha256: "00".repeat(32),
        format: "wav",
        sampleRate: 48000,
        channels: 1,
        frames: 48000,
      }),
    )
    .at({ bar: 1 });
  return { project, from, to, pattern, sample };
}

it.each([false, true])("keeps placement read-only and relocates membership with revision (restored=%s)", (restore) => {
  const initial = fixture();
  const project = restore ? Project.fromSnapshot(initial.project.snapshot()) : initial.project;
  const from = project.tracks.find((track) => track.clips.length)!;
  const to = project.tracks.find((track) => track !== from)!;
  const pattern = from.clips[0]!;
  const sample = from.sampleClips[0]!;
  const before = project.snapshot();
  for (const clip of [pattern, sample]) {
    expect(Reflect.set(clip, "track", to)).toBe(false);
    expect(Reflect.set(clip, "startBeat", 12)).toBe(false);
  }
  expect(project.snapshot()).toEqual(before);
  pattern.relocate(to, 12);
  sample.relocate(to, 12);
  expect(project.revisionBigInt).toBeGreaterThan(BigInt(before.revision));
  expect(from.clips).toEqual([]);
  expect(from.sampleClips).toEqual([]);
  expect(to.clips).toEqual([pattern]);
  expect(to.sampleClips).toEqual([sample]);
  expect(to.channelIds).toEqual(from.channelIds);
  expect(pattern.lastBeat).toBe(20);
  for (const clip of [pattern, sample]) {
    expect(clip.track).toBe(to);
    expect(clip.toSpec()).toMatchObject({ trackId: to.id, startBeat: { numerator: 12, denominator: 1 } });
  }
  expect(Project.fromSnapshot(project.snapshot()).snapshot()).toEqual(project.snapshot());
});

it("fits sample bars using its current position after relocation and time-signature edits", () => {
  const { project, sample, to } = fixture();
  project.addTimeSignature({ startBar: 3, numerator: 3, denominator: 4 });
  sample.relocate(to, 8);
  sample.fitBars(2);
  expect(sample.durationBeats).toBe(6);
  project.setTimeSignature(3, 4);
  project.addTimeSignature({ startBar: 3, numerator: 5, denominator: 4 });
  sample.fitBars(2);
  expect(sample.durationBeats).toBe(10);
});

it("rejects invalid clip moves without changing membership, routing or revision", () => {
  const { project, pattern, sample, to } = fixture();
  const foreign = new Project().addTrack();
  const before = project.snapshot();
  for (const clip of [pattern, sample]) {
    for (const [track, beat] of [
      [foreign, 4],
      [to, -1],
      [to, NaN],
    ] as const) {
      expect(() => clip.relocate(track, beat)).toThrow();
      expect(project.snapshot()).toEqual(before);
    }
  }
});
