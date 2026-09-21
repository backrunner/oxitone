import { expect, it } from "vitest";
import { Project, createAutomationNamespace } from "../src/index.js";
import type { AutomationRecordingEdit } from "@oxitone/protocol";

const config = { pluginId: `vst3.${"1".repeat(32)}`, pluginVersion: "1.0.0", parameters: { "0": 0.5 } };
function fixture() {
  const project = new Project();
  const channel = project.addChannel({ effectChain: [config] });
  const original = channel.effectInstances[0]!.param("0").automate(
    createAutomationNamespace().sine({ periodBeats: 4 }),
  );
  const edit: AutomationRecordingEdit = {
    target: {
      kind: "channelInsert",
      index: 0,
      slot: 0,
      pluginId: config.pluginId,
      pluginVersion: config.pluginVersion,
    },
    parameters: [{ id: 0, name: "Gain" }],
    spans: [
      { parameterId: 0, start: 2, end: 6, value: 0.2 },
      { parameterId: 0, start: 3, end: 4, value: 0.8 },
      { parameterId: 0, start: 8, end: 9, value: 0.6 },
    ],
  };
  return { project, original, edit };
}
it("preserves generators and maps later loop passes into independent local Playlist clips", () => {
  const { project, original, edit } = fixture();
  const before = project.snapshot();
  expect(project.recordAutomation(edit)).toBe(project);
  const snapshot = project.snapshot();
  expect(snapshot.channels).toEqual(before.channels);
  expect(snapshot.automation.find((lane) => lane.id === original.id)).toEqual(before.automation[0]);
  expect(snapshot.tracks).toHaveLength(1);
  expect(snapshot.tracks[0]!.name).toBe("Gain recording");
  expect(snapshot.automationClips!.map((clip) => [clip.startBeat, clip.durationBeats])).toEqual([
    [
      { numerator: 2, denominator: 1 },
      { numerator: 4, denominator: 1 },
    ],
    [
      { numerator: 8, denominator: 1 },
      { numerator: 1, denominator: 1 },
    ],
  ]);
  const lanes = snapshot.automation.filter((lane) => lane.id !== original.id);
  expect(lanes.every((lane) => lane.priority === 1 && lane.playback === "playlist" && lane.combine === "replace")).toBe(
    true,
  );
  expect(lanes[0]!.source).toMatchObject({
    kind: "curve",
    points: [
      { beat: { numerator: 0, denominator: 1 }, value: 0.2 },
      { beat: { numerator: 1, denominator: 1 }, value: 0.8 },
      { beat: { numerator: 2, denominator: 1 }, value: 0.2 },
    ],
  });
  project.recordAutomation(edit);
  expect(project.automationLanes.slice(-2).every((lane) => lane.priority === 2)).toBe(true);
  expect(Project.fromSnapshot(project.snapshot()).snapshot()).toEqual(project.snapshot());
});
it("rejects stale owners and invalid takes before mutation, and empty takes are noops", () => {
  const { project, edit } = fixture();
  const before = project.snapshot();
  for (const invalid of [
    { ...edit, target: { ...edit.target, index: 4 } },
    { ...edit, target: { ...edit.target, pluginVersion: "2" } },
    { ...edit, spans: [{ parameterId: 1, start: 0, end: 1, value: 0.2 }] },
    { ...edit, spans: [{ parameterId: 0, start: 0, end: 1, value: NaN }] },
    {
      ...edit,
      spans: Array.from({ length: 1025 }, (_, i) => ({ parameterId: 0, start: i * 2, end: i * 2 + 1, value: 0.2 })),
    },
  ]) {
    expect(() => project.recordAutomation(invalid)).toThrow();
    expect(project.snapshot()).toEqual(before);
  }
  project.recordAutomation({ ...edit, spans: [] });
  expect(project.snapshot()).toEqual(before);
});

it("includes recording layers in an existing solo group without changing prior Track controls", () => {
  for (const enabled of [false, true]) {
    const { project, edit } = fixture();
    const track = project.addTrack("Solo");
    track.solo = true;
    track.enabled = enabled;
    const before = track.toSpec();
    project.recordAutomation(edit);
    expect(track.toSpec()).toEqual(before);
    expect(project.tracks[1]!.solo).toBe(enabled);
  }
});
