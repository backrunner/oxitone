import { it, expect } from "vitest";
import { createEngine, dispose, getPluginInfo } from "../src/index.js";

it("accepts public hyphenated engine policies and emits public parameter smoothing names", () => {
  const engine = createEngine({
    allowPlugins: "signed-only",
    deviceRatePolicy: "adapt-device",
    deviceChangePolicy: "follow-default",
  });
  try {
    const info = getPluginInfo(engine, "oxitone.wavetable", "1.0.0");
    expect(info.parameters.some((spec) => spec.smoothing === "one-pole")).toBe(true);
    expect(() => getPluginInfo(engine, "missing", "1.0.0")).toThrow();
  } finally {
    dispose(engine);
  }
});
