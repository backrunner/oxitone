import { expect, it } from "vitest";
import { ErrorCode } from "@oxitone/protocol";
import { Project, createAutomationNamespace, effect, wavetable, orderEffects, pluginConfig } from "../src/index.js";

const source = createAutomationNamespace().constant(.25);
const conflict = expect.objectContaining({ code: ErrorCode.EditScopeConflict });

it("copies configurations without copying or reviving the previous owner's execution identity", () => {
  const project = new Project();
  const channel = project.addChannel({ effectChain: [effect("delay")] });
  const original = channel.effectInstances[0]!, saved = channel.effectChain[0]!;
  const clone = project.addChannel({ instrument: channel.instrument, effectChain: channel.effectChain });
  expect(clone.instrumentInstance.id).not.toBe(channel.instrumentInstance.id);
  expect(clone.effectInstances[0]!.id).not.toBe(original.id);
  expect(channel.addEffect(saved).id).not.toBe(original.id);
  expect(project.master.addEffect(saved).id).not.toBe(original.id);
  expect(pluginConfig("effect", saved).toSpec()).not.toHaveProperty("instanceId");
  channel.removeEffect(original);
  expect(channel.addEffect(saved).id).not.toBe(original.id);
  expect(() => original.config).toThrowError(expect.objectContaining({ code: ErrorCode.EditTargetMissing }));
});

it("creates independent instances from shared config and keeps host and plugin parameters separate", () => {
  const project = new Project(); const config = effect("delay", { feedback: .3 });
  const channel = project.addChannel({ instrument: wavetable({ level: .8 }), effectChain: [config, config], level: .6 });
  const [first, second] = channel.effectInstances;
  expect(first!.id).not.toBe(second!.id);
  expect(first).toBe(channel.effectInstances[0]);
  first!.param("feedback").set(.7);
  second!.host.param("mix").set(.2);
  second!.host.param("bypass").set(true);
  channel.instrumentInstance.param("level").set(.4);
  expect(channel.level).toBe(.6);
  expect(channel.instrument.parameters.level).toBe(.4);
  expect(channel.effectChain).toMatchObject([{ parameters: { feedback: .7 } }, { parameters: { feedback: .3 }, mix: .2, bypass: true }]);
  expect(config.parameters.feedback).toBe(.3);
  expect(first!.config.toSpec()).not.toHaveProperty("instanceId");
  expect(second!.host.param("mix").automate(source).target).toEqual({ entityId: second!.id, parameterId: "mix", scope: "effectHost" });
  expect(channel.instrumentInstance.param("level").automate(source).target.entityId).toBe(channel.instrumentInstance.id);
  expect(() => channel.instrumentInstance.host).toThrow();
});

it("reorders by object identity and rejects deletion/replacement until bound lanes are explicitly removed", () => {
  const project = new Project(); const bus = project.addMixerChannel({ inserts: [effect("delay"), effect("delay")] });
  const [first, second] = bus.effectInstances;
  const lane = first!.param("feedback").automate(source);
  bus.reorderEffects([second!, first!]);
  expect(bus.effectInstances).toEqual([second, first]);
  expect(project.snapshot().automation[0]!.target.entityId).toBe(first!.id);
  first!.param("feedback").set(.6);
  expect(bus.inserts[1]!.parameters.feedback).toBe(.6);
  const before = project.snapshot();
  expect(() => bus.removeEffect(first!)).toThrowError(conflict);
  expect(() => { bus.inserts = [effect("delay")]; }).toThrowError(conflict);
  expect(project.snapshot()).toEqual(before);
  project.removeAutomationLane(lane); bus.removeEffect(first!);
  expect(() => first!.param("feedback").set(.2)).toThrowError(expect.objectContaining({ code: ErrorCode.EditTargetMissing }));
  expect(() => first!.param("feedback").automate(source)).toThrow();
  expect(bus.addEffect(effect("delay")).id).not.toBe(first!.id);
});

it("rejects cross-owner/stale permutations, legacy index bindings and identity reuse by replacements", () => {
  const project = new Project(); const channel = project.addChannel({ effectChain: [effect("delay"), effect("delay")] });
  const [first, second] = channel.effectInstances;
  const other = new Project().addChannel({ effectChain: [effect("delay")] }).effectInstances[0]!;
  expect(() => channel.reorderEffects([other, second!])).toThrowError(conflict);
  expect(() => channel.reorderEffects([first!, first!])).toThrowError(conflict);
  expect(() => orderEffects(channel, [0, 0])).toThrowError(conflict);
  expect(() => orderEffects(channel, [0, 2])).toThrowError(conflict);
  expect(orderEffects(channel, [1, 0])).toBe(channel);
  expect(channel.effectInstances).toEqual([second, first]);
  orderEffects(channel, [1, 0]);
  expect(() => { channel.effectChain = [{ ...channel.effectChain[0]!, pluginVersion: "2.0.0" }, channel.effectChain[1]!]; }).toThrowError(conflict);
  const lane = channel.automate("insert.0.parameter.feedback", source);
  expect(() => channel.reorderEffects([second!, first!])).toThrowError(conflict);
  project.removeAutomationLane(lane); channel.reorderEffects([second!, first!]);
});

it("failed mutations preserve subsequent allocation and musical IDs, and typed snapshots restore exactly", () => {
  const build = () => {
    const project = new Project({ seed: 42 }); const channel = project.addChannel({ effectChain: [effect("delay")] });
    const lane = channel.effectInstances[0]!.param("feedback").automate(source);
    return { project, channel, lane };
  };
  const left = build(), right = build();
  expect(() => { left.channel.effectChain = [effect("delay")]; }).toThrowError(conflict);
  left.project.removeAutomationLane(left.lane); right.project.removeAutomationLane(right.lane);
  expect(left.channel.addEffect(effect("delay")).id).toBe(right.channel.addEffect(effect("delay")).id);
  left.channel.effectInstances[0]!.param("feedback").automate(source);
  const saved = left.project.snapshot();
  const restored = Project.fromSnapshot(saved);
  expect(restored.snapshot()).toEqual(saved);
  const instances = restored.channels[0]!.effectInstances;
  restored.channels[0]!.reorderEffects([...instances].reverse());
  expect(restored.automationLanes[0]!.target.entityId).toBe(instances[0]!.id);
  const noEffects = new Project({ seed: 42 }); noEffects.addChannel();
  const withEffects = new Project({ seed: 42 }); withEffects.addChannel({ effectChain: [effect("delay")] });
  expect(withEffects.addTrack().id).toBe(noEffects.addTrack().id);
});
