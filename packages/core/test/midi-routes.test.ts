import { expect, it } from "vitest";
import { Project, effect } from "../src/index.js";
import { decodeProjectSnapshot, encodeProjectSnapshot } from "@oxitone/protocol";

it("owns routes by instance, clones reads, preserves reordering and removes detached sources", () => {
  const project = new Project();
  const source = project.addChannel();
  const target = project.addChannel();
  const first = source.addEffect(effect("nonlinearFilter"));
  const second = source.addEffect(effect("delay"));
  source.routeMidi(first, [target]);
  source.routeMidi(source.instrumentInstance, [target]);
  const routes = source.midiRoutes;
  routes[first.id]!.length = 0;
  expect(source.midiRoutes[first.id]).toEqual([target.id]);
  source.reorderEffects([second, first]);
  expect(source.midiRoutes[first.id]).toEqual([target.id]);
  source.removeEffect(first);
  expect(source.midiRoutes[first.id]).toBeUndefined();
  const old = source.instrumentInstance;
  const retained = source.instrument;
  source.instrument = retained;
  expect(source.midiRoutes[old.id]).toEqual([target.id]);
  source.instrument = { pluginId: "oxitone.wavetable", pluginVersion: "1.0.0", parameters: {} };
  expect(source.midiRoutes[old.id]).toBeUndefined();
  expect(() => source.routeMidi(first, [target])).toThrow();
  target.mute = true;
  expect(() => encodeProjectSnapshot(project.snapshot())).not.toThrow();
});

it("rejects cross-project targets, duplicate destinations and cycles before changing revision", () => {
  const project = new Project();
  const a = project.addChannel();
  const b = project.addChannel();
  const c = project.addChannel();
  a.routeMidi(a.instrumentInstance, [b]);
  b.routeMidi(b.instrumentInstance, [c]);
  const before = project.snapshot();
  for (const destinations of [[a], [b, b], [new Project().addChannel()]])
    expect(() => a.routeMidi(a.instrumentInstance, destinations)).toThrow();
  expect(() => c.routeMidi(c.instrumentInstance, [a])).toThrow();
  expect(() => a.routeMidi(b.instrumentInstance, [c])).toThrow();
  expect(project.snapshot()).toEqual(before);
  a.routeMidi(a.instrumentInstance, []);
  expect(a.midiRoutes).toEqual({});
});

it("round-trips snapshots and rejects old versions and invalid recovery references without mutation", () => {
  const project = new Project();
  const a = project.addChannel();
  const b = project.addChannel();
  a.routeMidi(a.instrumentInstance, [b]);
  const snapshot = project.snapshot();
  expect(snapshot.protocolVersion).toBe("1.7");
  expect(Project.fromSnapshot(snapshot).snapshot()).toEqual(snapshot);
  expect(() => decodeProjectSnapshot(JSON.stringify({ ...snapshot, protocolVersion: "1.6" }))).toThrow();
  const bad = structuredClone(snapshot);
  bad.channels[0]!.midiRoutes![a.instrumentInstance.id] = ["chn_missing"];
  const saved = JSON.stringify(bad);
  expect(() => Project.fromSnapshot(bad)).toThrow();
  expect(JSON.stringify(bad)).toBe(saved);
});
