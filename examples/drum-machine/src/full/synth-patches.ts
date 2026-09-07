import { wavetable } from "@oxitone/core";

/** Source presets: envelopes carry attacks; frame motion animates sustained tones. */
export const crystalKeys = () => wavetable({
  oscA: { wave: "triangle", morphTo: "glass", position: 0.12, unison: 2, detune: 3, spread: 0.55, phaseSpread: 0.2 },
  oscB: { wave: "sine", octave: 1 }, mix: 0.12, fm: 0.12, filter: { cutoff: 4100, resonance: 0.03 },
  filterEnvelope: { amount: 12, attack: 0, decay: 0.35, sustain: 0, release: 0.2, decayCurve: -0.4 },
  amp: { attack: 0.004, decay: 0.65, sustain: 0.12, release: 0.5, decayCurve: -0.4, releaseCurve: -0.3 },
  modEnvelope: { attack: 0, decay: 0.22, sustain: 0, release: 0.12 },
  modulation: [{ source: "modEnv", target: "fm", amount: 0.18 }, { source: "velocity", target: "cutoff", amount: 0.08 }],
});
export const tapeGlass = () => wavetable({
  oscA: { bank: "digital", position: 0.35, unison: 3, detune: 6, spread: 0.7, phaseSpread: 0.5 },
  oscB: { wave: "sine", octave: 1 }, mix: 0.08, fm: 0.08, filter: { cutoff: 5200, resonance: 0.03 },
  amp: { attack: 0.003, decay: 0.35, sustain: 0.12, release: 0.28, decayCurve: -0.55 },
  lfo: { rateHz: 0.42, positionA: 0.1, pitch: 0.02 },
});
export const skyChords = () => wavetable({
  oscA: { wave: "saw", morphTo: "square", position: 0.09, unison: 7, detune: 19, spread: 0.95, phaseSpread: 0.73 },
  oscB: { wave: "saw", octave: 0, unison: 3, detune: 8, spread: 0.48, phase: 0.23, phaseSpread: 0.41 },
  mix: 0.3, noise: { level: 0.013 }, filter: { cutoff: 10500, resonance: 0.025 },
  filterEnvelope: { amount: 4, attack: 0.008, decay: 0.35, sustain: 0.5, release: 0.14 },
  amp: { attack: 0.007, decay: 0.18, sustain: 0.94, release: 0.1, releaseCurve: -0.5 },
  lfo: { rateHz: 140 / 60 / 8, positionA: 0.025 }, lfo2: { shape: "triangle", rateHz: 0.29 },
  modulation: [{ source: "lfo2", target: "positionB", amount: 0.04 },
    { source: "velocity", target: "cutoff", amount: 0.055 }, { source: "noteRandom", target: "pan", amount: 0.035 }],
});
export const chordBody = () => wavetable({
  oscA: { wave: "saw", morphTo: "triangle", position: 0.35, unison: 3, detune: 9, spread: 0.4, phaseSpread: 0.37 },
  filter: { cutoff: 3600, resonance: 0.025 },
  amp: { attack: 0.008, decay: 0.3, sustain: 0.65, release: 0.12, decayCurve: -0.3 },
});
export const horizonLead = () => wavetable({
  oscA: { wave: "saw", morphTo: "square", position: 0.27, unison: 3, detune: 6, spread: 0.24, phaseSpread: 0.37 },
  oscB: { wave: "triangle", octave: 1 }, mix: 0.13, filter: { cutoff: 6300, resonance: 0.06 },
  filterEnvelope: { amount: 7, attack: 0.003, decay: 0.2, sustain: 0.15, release: 0.2, decayCurve: -0.5 },
  amp: { attack: 0.007, decay: 0.18, sustain: 0.8, release: 0.13, releaseCurve: -0.4 },
  lfo: { rateHz: 5.1, pitch: 0.035, positionA: 0.035 },
  modEnvelope: { attack: 0, decay: 0.16, sustain: 0, release: 0.08 },
  modulation: [{ source: "modEnv", target: "positionA", amount: 0.1 },
    { source: "velocity", target: "cutoff", amount: 0.06 }],
});
export const subBass = () => wavetable({
  oscA: { level: 0 }, oscB: { level: 0 }, sub: { wave: "sine", octave: 0, level: 0.9 },
  voiceMode: "mono", amp: { attack: 0.005, decay: 0.1, sustain: 0.92, release: 0.045, releaseCurve: -0.4 },
});
export const motionBass = () => wavetable({
  oscA: { bank: "digital", position: 0.22, warpMode: "bend", warp: 0.22, unison: 2, detune: 5, spread: 0.18, phaseSpread: 0.14 },
  oscB: { wave: "sine", octave: 1 }, mix: 0.04, fm: 0.2,
  voiceMode: "mono", glide: 0.018, filter: { cutoff: 3800, resonance: 0.13 },
  filterEnvelope: { amount: 15, attack: 0.003, decay: 0.22, sustain: 0, release: 0.07, decayCurve: -0.5 },
  amp: { attack: 0.004, decay: 0.18, sustain: 0.6, release: 0.06, decayCurve: -0.4 },
  modEnvelope: { attack: 0.003, decay: 0.28, sustain: 0, release: 0.08, decayCurve: -0.5 },
  lfo2: { shape: "triangle", rateHz: 140 / 60 }, macros: [0.4],
  modulation: [{ source: "modEnv", target: "positionA", amount: 0.55, curve: 0.3 },
    { source: "modEnv", target: "fm", amount: 0.32 }, { source: "lfo2", target: "warpA", amount: 0.16 },
    { source: "macro1", target: "cutoff", amount: 0.1 }],
});
export const vowelBass = () => wavetable({
  oscA: { bank: "vowel", position: 0.14, warpMode: "bend", warp: 0.2 },
  oscB: { wave: "sine", octave: 1 }, mix: 0, fm: 0.06, filter: { cutoff: 5000, resonance: 0.08 },
  amp: { attack: 0.005, decay: 0.25, sustain: 0.15, release: 0.05, decayCurve: -0.4 },
  modEnvelope: { attack: 0.045, decay: 0.2, sustain: 0, release: 0.05 },
  modulation: [{ source: "modEnv", target: "positionA", amount: 0.8 }],
});
export const skyPad = () => wavetable({
  oscA: { bank: "analog", position: 0.25, unison: 5, detune: 17, spread: 1, phaseSpread: 0.73 },
  oscB: { wave: "triangle", octave: 1 }, mix: 0.08, noise: { level: 0.007 },
  filter: { cutoff: 2100, resonance: 0.03 }, amp: { attack: 0.6, decay: 0.3, sustain: 0.65, release: 0.7, attackCurve: -0.3 },
  lfo: { rateHz: 140 / 60 / 16, positionA: 0.2, cutoff: 3 },
});
export const noiseFx = () => wavetable({
  oscA: { level: 0 }, noise: { level: 0.5 }, filter: { type: "bandpass", cutoff: 3800, resonance: 0.07 },
  amp: { attack: 0.12, decay: 0.3, sustain: 0.7, release: 0.08 },
});
export const downlifter = () => wavetable({
  oscA: { level: 0 }, noise: { level: 0.5 }, filter: { type: "bandpass", cutoff: 2600, resonance: 0.05 },
  filterEnvelope: { amount: 20, attack: 0, decay: 0.85, sustain: 0, release: 0.4, decayCurve: -0.3 },
  amp: { attack: 0.003, decay: 0.9, sustain: 0, release: 0.7, decayCurve: -0.4 },
});
/** A short center transient under the wide saw, leaving its sustain to the hook. */
export const chordPluck = () => wavetable({
  oscA: { wave: "saw", morphTo: "triangle", position: 0.24, unison: 3, detune: 8, spread: 0.6, phaseSpread: 0.31 },
  oscB: { wave: "sine", octave: 1 }, mix: 0.06, fm: 0.07,
  filter: { cutoff: 1100, resonance: 0.12 },
  filterEnvelope: { amount: 25, attack: 0.002, decay: 0.18, sustain: 0, release: 0.09, decayCurve: -0.6 },
  amp: { attack: 0.003, decay: 0.23, sustain: 0.04, release: 0.12, decayCurve: -0.6 },
  modulation: [{ source: "velocity", target: "fm", amount: 0.05 }],
});
export const reeseBass = () => wavetable({
  oscA: { wave: "saw", unison: 3, detune: 12, spread: 0.24, phaseSpread: 0.43 },
  oscB: { wave: "square", octave: 0, level: 0.4 }, mix: 0.14,
  voiceMode: "mono", glide: 0.04, filter: { cutoff: 780, resonance: 0.07 },
  amp: { attack: 0.025, decay: 0.35, sustain: 0.7, release: 0.1 },
  lfo: { rateHz: 140 / 60 / 8, cutoff: 5 },
  lfo2: { shape: "triangle", rateHz: 140 / 60 / 4 },
  modulation: [{ source: "lfo2", target: "positionA", amount: 0.04 }],
});
export const halo = () => wavetable({
  oscA: { wave: "triangle", morphTo: "glass", position: 0.35, unison: 4, detune: 11, spread: 1, phaseSpread: 0.61 },
  oscB: { wave: "sine", octave: 1 }, mix: 0.18, fm: 0.09,
  filter: { cutoff: 6800, resonance: 0.02 },
  amp: { attack: 0.22, decay: 0.6, sustain: 0.35, release: 0.45 },
  lfo: { rateHz: 140 / 60 / 16, positionA: 0.13 },
  modulation: [{ source: "lfo1", target: "pan", amount: 0.2 }],
});
