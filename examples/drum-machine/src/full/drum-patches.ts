import { wavetable } from "@oxitone/core";

/** Separate transient/body/tail layers, synthesized by Rust rather than stacked full kicks. */
export const kickAttack = () => wavetable({
  oscA: { level: 0 }, noise: { level: 0.85 }, filter: { type: "highpass", cutoff: 1800, resonance: 0.04 },
  amp: { attack: 0.0005, decay: 0.012, sustain: 0, release: 0.008, decayCurve: -0.3 },
});
export const snareBody = () => wavetable({
  oscA: { wave: "sine", phase: 0 }, oscB: { wave: "triangle", pitch: 7, level: 0.25 }, mix: 0.15,
  noise: { level: 0.055 }, filter: { cutoff: 2300, resonance: 0.04 },
  amp: { attack: 0.001, decay: 0.24, sustain: 0, release: 0.06, decayCurve: -0.22 },
  modEnvelope: { attack: 0, decay: 0.025, sustain: 0, release: 0.015 },
  modulation: [{ source: "modEnv", target: "pitch", amount: 0.46 }],
});
export const snareTail = () => wavetable({
  oscA: { level: 0 }, noise: { level: 0.95 }, filter: { type: "highpass", cutoff: 1250, resonance: 0.04 },
  amp: { attack: 0.003, decay: 0.13, sustain: 0.48, release: 0.055, decayCurve: -0.15 },
});
export const rideCymbal = () => wavetable({
  oscA: { wave: "square", level: 0.13 }, oscB: { wave: "square", pitch: 6.7, level: 0.1 }, mix: 0.4,
  ring: 0.7, noise: { level: 0.5 }, filter: { type: "highpass", cutoff: 4900, resonance: 0.03 },
  amp: { attack: 0.001, decay: 0.24, sustain: 0, release: 0.11, decayCurve: -0.2 },
});
export const shaker = () => wavetable({
  oscA: { level: 0 }, noise: { level: 0.65 }, filter: { type: "highpass", cutoff: 6100, resonance: 0.03 },
  amp: { attack: 0.005, decay: 0.035, sustain: 0, release: 0.018 },
});
export const tomFill = () => wavetable({
  oscA: { wave: "sine" }, oscB: { wave: "triangle", pitch: 7, level: 0.2 }, mix: 0.18,
  filter: { cutoff: 1700, resonance: 0.04 }, noise: { level: 0.025 },
  amp: { attack: 0.001, decay: 0.19, sustain: 0, release: 0.055 },
  modEnvelope: { attack: 0, decay: 0.032, sustain: 0, release: 0.02 },
  modulation: [{ source: "modEnv", target: "pitch", amount: 0.4 }],
});
export const crashCymbal = () => wavetable({
  oscA: { wave: "square", level: 0.1 }, oscB: { wave: "square", pitch: 6.7, level: 0.08 }, mix: 0.45,
  ring: 0.6, noise: { level: 0.8 }, filter: { type: "highpass", cutoff: 3100, resonance: 0.03 },
  amp: { attack: 0.002, decay: 0.8, sustain: 0.08, release: 0.4, decayCurve: -0.2 },
});
export const reverseCymbal = () => wavetable({
  oscA: { level: 0 }, noise: { level: 0.75 }, filter: { type: "highpass", cutoff: 2200, resonance: 0.03 },
  amp: { attack: 0.38, decay: 0, sustain: 1, release: 0.012, attackCurve: 0.45 },
});
export const buildRiser = () => wavetable({
  oscA: { wave: "saw", unison: 3, detune: 21, spread: 0.85 },
  oscB: { wave: "square", octave: 1, level: 0.2 }, mix: 0.14, filter: { cutoff: 2000, resonance: 0.12 },
  amp: { attack: 0.08, decay: 0, sustain: 0.8, release: 0.04 },
  lfo: { shape: "ramp", rateHz: 140 / 60 * 4, level: 0.65 },
});
