import { expect, it } from "vitest";
import { vst3Config } from "../src/project.js";

it("accepts parameters declared by a captured configuration and still rejects unrelated overrides", () => {
  const classId = "1".repeat(32),
    sha256 = "a".repeat(64);
  const plugin = {
    registrationVersion: 1 as const,
    pluginId: `vst3.${classId}`,
    pluginVersion: `0.0.0+${sha256}`,
    sha256,
    kind: "effect" as const,
    parameters: [],
  };
  const configuration = { formatVersion: 1 as const, classId, sha256, stateBase64: "AQID", parameters: { "7": 0.5 } };
  expect(vst3Config(plugin, { configuration, parameters: { "7": 0.25 } })).toMatchObject({
    state: configuration,
    parameters: { "7": 0.25 },
  });
  expect(() => vst3Config(plugin, { parameters: { "7": 0.25 } })).toThrow("Unknown or read-only");
  expect(() => vst3Config(plugin, { configuration, parameters: { "8": 0.25 } })).toThrow("Unknown or read-only");
  expect(() => vst3Config(plugin, { configuration: { ...configuration, classId: "2".repeat(32) } })).toThrow(
    "different binary/class",
  );
  expect(configuration.parameters).toEqual({ "7": 0.5 });
});
