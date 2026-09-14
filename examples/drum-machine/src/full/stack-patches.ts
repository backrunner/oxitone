import { wavetable } from "@oxitone/core";

/** Bright pulse texture sits above the main saw stack without adding another low root. */
export const pulseChords = () =>
  wavetable({
    oscA: { wave: "square", morphTo: "saw", position: 0.3, unison: 4, detune: 13, spread: 0.9, phaseSpread: 0.63 },
    oscB: { bank: "digital", position: 0.17, octave: 1, level: 0.2 },
    mix: 0.12,
    filter: { cutoff: 10500, resonance: 0.04 },
    amp: { attack: 0.006, decay: 0.2, sustain: 0.72, release: 0.08 },
    lfo: { rateHz: 140 / 60 / 2, positionA: 0.12 },
  });
/** A short distorted harmonic stab gives chord attacks a different character from the pad. */
export const chordEdge = () =>
  wavetable({
    oscA: { bank: "digital", position: 0.57 },
    oscB: { wave: "saw", level: 0.55, octave: 0 },
    mix: 0.3,
    filter: { cutoff: 2700, resonance: 0.12 },
    filterEnvelope: { amount: 14, attack: 0.002, decay: 0.19, sustain: 0.1, release: 0.07 },
    amp: { attack: 0.003, decay: 0.21, sustain: 0.2, release: 0.06 },
    modEnvelope: { attack: 0, decay: 0.24, sustain: 0, release: 0.06 },
    modulation: [{ source: "modEnv", target: "positionA", amount: 0.31 }],
  });
