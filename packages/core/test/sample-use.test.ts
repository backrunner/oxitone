import { expect, it } from "vitest";
import { Project, createAutomationNamespace } from "../src/index.js";
import type { SampleUse } from "@oxitone/protocol";
const sample: Exclude<SampleUse["sample"], number> = {
  assetUri: "assets/kick.wav",
  sha256: "a".repeat(64),
  format: "wav",
  sampleRate: 48000,
  channels: 1,
  frames: "4800",
};
it("places imported audio on existing or new tracks and keeps resources immutable", () => {
  const p = new Project();
  p.addTrack("Audio");
  p.useSample({ sample, destination: { kind: "arrangement", track: 0, startBeat: 2.5 } });
  p.useSample({ sample: 0, destination: { kind: "arrangement", startBeat: 8, name: "Drop" } });
  expect(p.samples).toHaveLength(1);
  expect(p.tracks).toHaveLength(2);
  expect(p.sampleClips.map((c) => c.toSpec())).toMatchObject([
    { tempoSync: "off", startBeat: { numerator: 5, denominator: 2 } },
    { tempoSync: "off" },
  ]);
});
it("replaces one resource while retaining instance identity, controls and automation", () => {
  const p = new Project();
  const old = p.importSampleRef({ ...sample, id: "old" });
  const channel = p.addChannel({
    instrument: {
      pluginId: "oxitone.sampler",
      pluginVersion: "1.0.0",
      parameters: { level: 0.6 },
      resources: { sample: old.id },
    },
  });
  const instance = channel.instrumentInstance;
  instance.param("level").automate(createAutomationNamespace().constant(0.4));
  const lanes = p.snapshot().automation;
  p.useSample({
    sample: { ...sample, sha256: "b".repeat(64) },
    destination: { kind: "plugin", owner: "channel", index: 0, resource: "sample" },
  });
  expect(channel.instrumentInstance.id).toBe(instance.id);
  expect(channel.instrument.parameters.level).toBe(0.6);
  expect(channel.instrument.resources?.sample).toBe(p.samples[1]!.id);
  expect(p.snapshot().automation).toEqual(lanes);
});
it("rejects unsupported slots and missing tracks atomically", () => {
  const p = new Project();
  p.addChannel();
  const before = p.snapshot();
  expect(() =>
    p.useSample({ sample, destination: { kind: "plugin", owner: "channel", index: 0, resource: "sample" } }),
  ).toThrow();
  expect(p.snapshot()).toEqual(before);
  expect(() => p.useSample({ sample, destination: { kind: "arrangement", track: 9, startBeat: 0 } })).toThrow();
  expect(p.snapshot()).toEqual(before);
});
it("retains Convolver host controls and rejects Slicer markers and unsupported VST3 resources", () => {
  const p = new Project();
  const old = p.importSampleRef({ ...sample, id: "old" });
  const effect = p.master.addEffect({
    pluginId: "oxitone.convolver",
    pluginVersion: "1.0.0",
    parameters: {},
    resources: { impulse: old.id },
    mix: 0.3,
    bypass: true,
  });
  p.useSample({ sample, destination: { kind: "plugin", owner: "bus", index: 0, slot: 0, resource: "impulse" } });
  expect(effect.config.toSpec()).toMatchObject({ mix: 0.3, bypass: true, resources: { impulse: p.samples[1]!.id } });
  p.addChannel({
    instrument: {
      pluginId: "oxitone.slicer",
      pluginVersion: "1.0.0",
      parameters: {},
      state: { sampleId: old.id, slices: [{ start: 0, end: 4800 }] },
    },
  });
  const before = p.snapshot();
  expect(() =>
    p.useSample({ sample, destination: { kind: "plugin", owner: "channel", index: 0, resource: "state.sampleId" } }),
  ).toThrow(/slice markers/);
  expect(p.snapshot()).toEqual(before);
  p.addChannel({ instrument: { pluginId: "vst3.test", pluginVersion: "1", parameters: {} } });
  const beforeVst = p.snapshot();
  expect(() =>
    p.useSample({ sample, destination: { kind: "plugin", owner: "channel", index: 1, resource: "sample" } }),
  ).toThrow(/VST3/);
  expect(p.snapshot()).toEqual(beforeVst);
});
