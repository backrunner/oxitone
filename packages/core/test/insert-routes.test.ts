import { expect, it } from "vitest";
import { Project, effect } from "../src/index.js";

it("keeps routes on stable instances through reorder, configuration edits and restore", () => {
  const project = new Project();
  const bus = project.addMixerChannel({ inserts: [effect("delay"), effect("delay")] });
  const source = project.addMixerChannel();
  const destination = project.addMixerChannel();
  const [first, second] = bus.effectInstances;
  const route = { inputs: { "1": source.id }, outputs: { "2": destination.id } };
  bus.routeInsert(first!, route);
  route.inputs["1"] = destination.id;
  bus.insertRoutes[first!.id]!.outputs!["2"] = source.id;
  expect(bus.insertRoutes[first!.id]).toEqual({ inputs: { "1": source.id }, outputs: { "2": destination.id } });
  bus.reorderEffects([second!, first!]);
  first!.param("feedback").set(0.25);
  expect(Object.keys(bus.insertRoutes)).toEqual([first!.id]);
  expect(Project.fromSnapshot(project.snapshot()).snapshot()).toEqual(project.snapshot());
  const clone = project.addMixerChannel({ inserts: bus.inserts });
  expect(clone.insertRoutes).toEqual({});
  bus.removeEffect(second!);
  expect(Object.keys(bus.insertRoutes)).toEqual([first!.id]);
  bus.routeInsert(first!);
  expect(bus.insertRoutes).toEqual({});
  bus.routeInsert(first!, { outputs: { "1": "mix_master" } });
  bus.removeEffect(first!);
  expect(bus.insertRoutes).toEqual({});
});

it("rejects invalid endpoints, bus indices and foreign owners atomically", () => {
  const project = new Project();
  const bus = project.addMixerChannel({ inserts: [effect("delay")] });
  const source = project.addMixerChannel({ inserts: [effect("delay")] });
  const first = bus.effectInstances[0]!;
  const before = project.snapshot();
  for (const routing of [
    { inputs: { "0": source.id } },
    { inputs: { "01": source.id } },
    { outputs: { "16": source.id } },
    { inputs: { "1": "mix_master" } },
    { outputs: { "1": bus.id } },
    { inputs: { "1": "mix_missing" } },
  ]) {
    expect(() => bus.routeInsert(first, routing)).toThrow();
    expect(project.snapshot()).toEqual(before);
  }
  expect(() => bus.routeInsert(source.effectInstances[0]!, { inputs: { "1": source.id } })).toThrow();
  const master = project.master.addEffect(effect("delay"));
  project.master.routeInsert(master, { inputs: { "1": source.id } });
  expect(() => project.master.routeInsert(master, { outputs: { "1": source.id } })).toThrow();
  const corrupt = project.snapshot();
  corrupt.mixerChannels.find((b) => b.id === bus.id)!.insertRoutes = { [first.id]: { inputs: { "1": "mix_missing" } } };
  expect(() => Project.fromSnapshot(corrupt)).toThrow();
});
