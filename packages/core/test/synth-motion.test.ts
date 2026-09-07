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
it("maps independent octave, level, Sub waves and the bounded modulation matrix", () => {
  for (const [index, wave] of (["sine", "triangle", "saw", "square", "pulse", "rounded"] as const).entries()) {
    expect(wavetable({ oscA: { octave: -4, level: 0 }, oscB: { octave: 4 }, sub: { wave, octave: 3, level: 0.5 } }).parameters)
      .toEqual({ "oscA.octave": -4, "oscA.level": 0, "oscB.octave": 4, "sub.wave": index, "sub.octave": 3, "sub.level": 0.5 });
  }
  expect(wavetable({ oscA: { bank: "digital", warpMode: "sync", warp: 0.4, unison: 16 },
    amp: { decayCurve: -0.5 }, lfo2: { shape: "ramp" }, macros: [0.8], fm: 0.3,
    modulation: [{ source: "macro1", target: "fm", amount: -0.4, curve: 0.2 }] }).parameters)
    .toMatchObject({ "oscA.bank": 2, "oscA.warpMode": 3, "oscA.unison": 16, "amp.decayCurve": -0.5,
      "lfo2.shape": 2, "macro1": 0.8, "mod.0.source": 9, "mod.0.target": 6, "mod.0.amount": -0.4, "mod.0.curve": 0.2 });
});
it("rejects invalid waveform, unbounded sources and invalid modulation ranges", () => {
  for (const options of [{ oscA: { morphTo: "unknown" } }, { oscA: { position: 1.1 } },
    { sub: { octave: -1.5 } }, { sub: { wave: "glass" } }, { sub: { octave: 5 } },
    { oscA: { octave: -5 } }, { oscB: { octave: 0.5 } }, { oscA: { level: -0.1 } },
    { modulation: Array.from({ length: 9 }, () => ({ source: "lfo1", target: "fm", amount: 0.1 })) },
    { lfo: { rateHz: 0 } }, { lfo: { rateHz: Infinity } },
    { lfo: { cutoff: 49 } }, { lfo: { positionA: -1.01 } }, { noise: { level: 2 } }]) {
    expect(() => wavetable(options as WavetableOptions)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
  }
});
