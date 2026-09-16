import { expect, it } from "vitest";
import { Project, pluginConfig, wavetable, effect } from "../src/index.js";

it("derives immutable plugin and host settings without mutating factories, resources or structured state", () => {
  const input = {
    pluginId: "fixture.synth",
    pluginVersion: "1.2.3",
    parameters: { "filter.cutoff": 300, level: 0.7 },
    resources: { sample: "smp_1" },
    state: { version: 1, values: [1, 2] },
  };
  const base = pluginConfig("instrument", input),
    variant = base.withParameters({ "filter.cutoff": 500 });
  input.state.values.push(3);
  input.resources.sample = "smp_2";
  expect(variant.toSpec()).toEqual({
    ...input,
    parameters: { ...input.parameters, "filter.cutoff": 500 },
    resources: { sample: "smp_1" },
    state: { version: 1, values: [1, 2] },
  });
  expect(base.parameters["filter.cutoff"]).toBe(300);
  expect(() => {
    // @ts-expect-error Frozen configuration maps are also read-only to TypeScript callers.
    variant.parameters.level = 0;
  }).toThrow();
  expect(() => base.withHost({ mix: 0.2 })).toThrow();
  expect(() => base.withParameters({ level: NaN })).toThrow();
  expect(() => pluginConfig("instrument", { ...input, state: { bad: () => 1 } })).toThrow();
  const fx = pluginConfig("effect", effect("delay", { feedback: 0.3 }, { mix: 0.4 }))
    .withParameters({ feedback: 0.6 })
    .withHost({ bypass: true });
  const project = new Project();
  const channel = project.addChannel({
    instrument: pluginConfig("instrument", wavetable()).withParameters({ level: 0.5 }),
    effectChain: [fx],
  });
  expect(channel.toSpec().instrument.parameters.level).toBe(0.5);
  expect(channel.toSpec().effectChain[0]).toMatchObject({ parameters: { feedback: 0.6 }, mix: 0.4, bypass: true });
});

it("source identity tracking does not change defensive-copy semantics when appending effects", () => {
  const project = new Project();
  const input = effect("delay", { feedback: 0.3 });
  const channel = project.addChannel({ effectChain: [input] });
  const bus = project.addMixerChannel({ inserts: [input] });
  input.parameters.feedback = 0.8;
  channel.addEffect(effect("delay"));
  bus.addEffect(effect("delay"));
  expect(channel.effectChain[0]!.parameters.feedback).toBe(0.3);
  expect(bus.inserts[0]!.parameters.feedback).toBe(0.3);
});
