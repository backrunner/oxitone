import { wavetable } from "@oxitone/core";

/** Audible second/third harmonics above a separate clean sine sub. */
export const foundationBass = () => wavetable({
  oscA: { wave: "saw", morphTo: "triangle", position: 0.6 },
  oscB: { wave: "square", octave: 1, level: 0.35 }, mix: 0.12,
  voiceMode: "mono", glide: 0.02, filter: { cutoff: 580, resonance: 0.03 },
  amp: { attack: 0.004, decay: 0.16, sustain: 0.86, release: 0.045 },
});
/** FM-only carrier retains far more partials than simultaneous FM + phase warp. */
export const turbineBass = () => wavetable({
  oscA: { bank: "digital", position: 0.43, unison: 2, detune: 4, spread: 0.25 },
  oscB: { wave: "sine", octave: 2, level: 0 }, mix: 0, fm: 0.16,
  voiceMode: "mono", glide: 0.025, filter: { cutoff: 6200, resonance: 0.06 },
  amp: { attack: 0.003, decay: 0.25, sustain: 0.72, release: 0.045, decayCurve: -0.45 },
  modEnvelope: { attack: 0.018, decay: 0.22, sustain: 0.14, release: 0.06, decayCurve: -0.4 },
  lfo2: { shape: "ramp", rateHz: 140 / 60 * 2 },
  modulation: [{ source: "modEnv", target: "fm", amount: 0.56 },
    { source: "modEnv", target: "positionA", amount: 0.43 },
    { source: "lfo2", target: "cutoff", amount: 0.16 }],
});
/** Harmonic vowel motion carries the long answer; dry center stays stable. */
export const talkingBass = () => wavetable({
  oscA: { bank: "vowel", position: 0.05 },
  oscB: { wave: "saw", octave: 1, level: 0.28 }, mix: 0.12,
  voiceMode: "mono", filter: { cutoff: 6500, resonance: 0.08 },
  amp: { attack: 0.004, decay: 0.2, sustain: 0.7, release: 0.04 },
  modEnvelope: { attack: 0.095, decay: 0.15, sustain: 0.1, release: 0.04, attackCurve: -0.25 },
  modulation: [{ source: "modEnv", target: "positionA", amount: 0.9 },
    { source: "modEnv", target: "cutoff", amount: 0.22 }],
});
/** Sync-warp-only stab provides a third spectral character without FM's extra mip reduction. */
export const laserBass = () => wavetable({
  oscA: { wave: "saw", warpMode: "sync", warp: 0.12 },
  oscB: { wave: "square", octave: 1, level: 0.2 }, mix: 0.1,
  voiceMode: "mono", filter: { cutoff: 4700, resonance: 0.14 },
  amp: { attack: 0.002, decay: 0.14, sustain: 0.25, release: 0.03, decayCurve: -0.55 },
  modEnvelope: { attack: 0, decay: 0.17, sustain: 0, release: 0.025, decayCurve: -0.65 },
  modulation: [{ source: "modEnv", target: "warpA", amount: 0.55 },
    { source: "modEnv", target: "pitch", amount: 0.08 }],
});
export const snareClap = () => wavetable({
  oscA: { level: 0 }, noise: { level: 0.8 },
  filter: { type: "bandpass", cutoff: 2100, resonance: 0.06 },
  amp: { attack: 0.001, decay: 0.1, sustain: 0, release: 0.06, decayCurve: -0.6 },
});
