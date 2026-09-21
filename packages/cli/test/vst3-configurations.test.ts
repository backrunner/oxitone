import { expect, it } from "vitest";
import { type Vst3Configuration, type Vst3Info, type Vst3Preset } from "@oxitone/protocol";
import { Vst3Configurations } from "../src/source/plugins/vst3-configurations.js";

const configuration: Vst3Configuration = {
  formatVersion: 1,
  classId: "1".repeat(32),
  sha256: "a".repeat(64),
  stateBase64: "AQID",
  parameters: { "9": 0.2 },
};
function preset(value = configuration): Vst3Preset {
  return {
    formatVersion: 1,
    kind: "oxitone-vst3-preset",
    name: "Gain",
    source: { bundlePath: "/local/Gain.vst3", classId: value.classId },
    configuration: value,
  };
}
const info: Vst3Info = {
  protocolVersion: 1,
  classId: configuration.classId,
  sha256: configuration.sha256,
  name: "Gain",
  vendor: "Fixture",
  version: "1",
  category: "Fx",
  inputChannels: 2,
  outputChannels: 2,
  audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [{ channels: 2, active: true }] },
  noteInput: false,
  noteOutput: false,
  configuration: { ...configuration, stateBase64: "BAUG", parameters: { "9": 0.5 } },
  parameters: [
    { id: 9, name: "Gain", unit: "", value: 0.5, default: 0.5, stepCount: 0, canAutomate: true, readOnly: false },
  ],
};
it("keeps imported opaque state across refresh and validates identity and parameter writability before activating", () => {
  const states = new Vst3Configurations();
  states.load("gain", preset(), "/preset.json");
  expect(states.get("gain")).toBeUndefined();
  expect(states.retained("gain")).toEqual(configuration);
  for (const changed of [
    { ...info, sha256: "b".repeat(64) },
    { ...info, classId: "2".repeat(32) },
    { ...info, parameters: [] },
    { ...info, parameters: [{ ...info.parameters[0]!, readOnly: true }] },
  ]) {
    expect(() => states.capture("gain", changed)).toThrow();
    expect(states.get("gain")).toBeUndefined();
    expect(states.path("gain")).toBe("/preset.json");
  }
  const restored = { ...info, configuration };
  expect(states.capture("gain", restored).parameters[0]!.value).toBe(0.2);
  expect(states.get("gain")!.stateBase64).toBe("AQID");
  states.refresh();
  expect(states.get("gain")).toBeUndefined();
  expect(states.capture("gain", restored).configuration).toEqual(configuration);
  states.remove("gain");
  expect(states.path("gain")).toBeUndefined();
  expect(states.capture("gain", info).configuration).toEqual(info.configuration);
});
it("retains the imported-state budget on refresh and preserves a prior preset if replacement exceeds it", () => {
  const states = new Vst3Configurations();
  const large = { ...configuration, stateBase64: Buffer.alloc(4 * 1024 * 1024).toString("base64") };
  for (let i = 0; i < 4; i++) states.load(String(i), preset(large), `/preset-${i}.json`);
  states.load("last", preset(), "/last.json");
  states.refresh();
  expect(() => states.load("last", preset(large), "/replacement.json")).toThrow(/budget/);
  expect(states.path("last")).toBe("/last.json");
  expect(states.capture("0", { ...info, configuration: large }).configuration).toEqual(large);
  expect(states.capture("0", { ...info, configuration: structuredClone(large) }).configuration).toBe(large);
  expect(() => states.capture("new", { ...info, configuration: large })).toThrow(/budget/);
  states.remove("1");
  expect(states.capture("0", { ...info, configuration: large }).configuration).toEqual(large);
  expect(states.capture("last", { ...info, configuration }).configuration).toEqual(configuration);
});
it("publishes the resolved parameter table and state while preserving the original recovery preset", () => {
  const states = new Vst3Configurations();
  states.load("gain", preset(), "/preset.json");
  const resolved = {
    ...info,
    parameters: [{ ...info.parameters[0]!, id: 17, name: "New parameter", value: 0.7 }],
    configuration: { ...configuration, stateBase64: "BAUG", parameters: { "17": 0.7 } },
  };
  expect(states.capture("gain", resolved)).toEqual(resolved);
  expect(states.get("gain")).toEqual(resolved.configuration);
  states.refresh();
  expect(states.retained("gain")).toEqual(configuration);
  expect(states.path("gain")).toBe("/preset.json");
});
