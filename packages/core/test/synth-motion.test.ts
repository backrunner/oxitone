import { expect, it } from "vitest";
import { ErrorCode, type WavetableOptions } from "@oxitone/protocol";
import { wavetable } from "../src/index.js";

it("maps native timbre and modulation controls without changing omitted defaults", () => {
  expect(wavetable().parameters).toEqual({});
  const options: WavetableOptions = { oscA: { wave: "organ", morphTo: "glass", position: 0.4, phase: 0.25, phaseSpread: 0.7 },
    sub: { level: 0.3, octave: -2 }, noise: { level: 0.1 }, lfo: { shape: "triangle", rateHz: 2.5, pitch: 0.1, cutoff: 12, positionA: 0.3, positionB: -0.2, level: 0.4 } };
  expect(wavetable(options).parameters).toEqual({ "oscA.wavetable": 4, "oscA.morphTo": 5, "oscA.position": 0.4,
    "oscA.phase": 0.25, "oscA.phaseSpread": 0.7, "sub.level": 0.3, "sub.octave": -2, "noise.level": 0.1,
    "lfo.shape": 1, "lfo.rateHz": 2.5, "lfo.pitch": 0.1, "lfo.cutoff": 12, "lfo.positionA": 0.3, "lfo.positionB": -0.2, "lfo.level": 0.4 });
});
it("rejects invalid waveform, unbounded sources and invalid modulation ranges", () => {
  for (const options of [{ oscA: { morphTo: "unknown" } }, { oscA: { position: 1.1 } },
    { sub: { octave: -1.5 } }, { lfo: { rateHz: 0 } }, { lfo: { rateHz: Infinity } },
    { lfo: { cutoff: 49 } }, { lfo: { positionA: -1.01 } }, { noise: { level: 2 } }]) {
    expect(() => wavetable(options as WavetableOptions)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
  }
});
