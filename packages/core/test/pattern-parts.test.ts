import { expect, it } from "vitest";
import { Pattern, Project } from "../src/index.js";

it("registers independent Channel parts atomically and restores their source identity", () => {
  const project = new Project();
  const a = project.addChannel(), b = project.addChannel();
  const melody = new Pattern({ lengthBeats: 4, notes: [{ pitch: 60, start: 0, duration: 1, velocity: .8 }] });
  const bass = melody.transpose(-12);
  const pattern = new Pattern({ lengthBeats: 4, parts: [{ channelId: a.id, pattern: melody }, { channelId: b.id, pattern: bass }] });
  project.addTrack().add(pattern).at({ bar: 1 });
  const snapshot = project.snapshot();
  expect(snapshot.patterns).toHaveLength(3);
  expect(Project.fromSnapshot(snapshot).snapshot()).toEqual(snapshot);
  expect(project.tracks[0]!.channelIds).toEqual([]);
  expect(() => pattern.transpose(12)).toThrow(/Select a Channel/);
  expect(() => new Pattern({ lengthBeats: 4, parts: [{ channelId: a.id, pattern: melody }, { channelId: a.id, pattern: bass }] })).toThrow();
  const invalid = new Pattern({ lengthBeats: 4, parts: [{ channelId: "chn_missing", pattern: melody }] });
  expect(() => project.tracks[0]!.add(invalid).at({ bar: 2 })).toThrow();
  expect(project.snapshot()).toEqual(snapshot);
});
