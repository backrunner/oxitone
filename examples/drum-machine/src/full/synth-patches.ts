import { wavetable } from "@oxitone/core";

/** Reusable source presets: slow motion for lofi, note-triggered motion for the drops. */
export const warmBass = () => wavetable({ oscA: { wave: "sine", morphTo: "organ", position: 0.16, phase: 0.25 },
  oscB: { wave: "triangle" }, mix: 0.1, filter: { cutoff: 620, resonance: 0.03 },
  amp: { attack: 0.01, decay: 0.22, sustain: 0.65, release: 0.12 } });
export const tapeGlass = () => wavetable({ oscA: { wave: "organ", morphTo: "glass", position: 0.34, unison: 2, detune: 5, phaseSpread: 0.22, spread: 0.45 },
  oscB: { wave: "sine", pitch: 12 }, mix: 0.08, noise: { level: 0.007 },
  filter: { cutoff: 3500, resonance: 0.03 }, filterEnvelope: { amount: 6, attack: 0.001, decay: 0.32, sustain: 0, release: 0.18 },
  amp: { attack: 0.008, decay: 0.45, sustain: 0.18, release: 0.3 },
  lfo: { shape: "sine", rateHz: 0.42, pitch: 0.035, positionA: 0.07 } });
export const eveningPad = () => wavetable({ oscA: { wave: "triangle", morphTo: "organ", position: 0.24, unison: 3, detune: 8, phaseSpread: 0.55, spread: 0.85 },
  filter: { cutoff: 1500, resonance: 0.05 }, noise: { level: 0.005 },
  amp: { attack: 0.55, decay: 0.3, sustain: 0.64, release: 0.6 },
  lfo: { shape: "sine", rateHz: 80 / 60 / 16, positionA: 0.16, cutoff: 2, level: 0.08 } });
export const skyChords = () => wavetable({ oscA: { wave: "saw", morphTo: "organ", position: 0.16, unison: 7, detune: 19, spread: 1, phaseSpread: 0.73 },
  oscB: { wave: "saw", morphTo: "glass", position: 0.13, pitch: 12, unison: 3, detune: 11, spread: 0.8, phase: 0.14, phaseSpread: 0.5 },
  mix: 0.2, noise: { level: 0.01 }, filter: { cutoff: 6200, resonance: 0.04 },
  filterEnvelope: { amount: 9, attack: 0.008, decay: 0.32, sustain: 0.2, release: 0.18 },
  amp: { attack: 0.016, decay: 0.22, sustain: 0.73, release: 0.17 },
  lfo: { shape: "sine", rateHz: 140 / 60 / 4, positionA: 0.1, positionB: -0.08 } });
export const horizonLead = () => wavetable({ oscA: { wave: "saw", morphTo: "glass", position: 0.25, unison: 5, detune: 8, spread: 0.75, phaseSpread: 0.62 },
  oscB: { wave: "triangle", morphTo: "organ", position: 0.3, pitch: 12, phase: 0.2 }, mix: 0.18,
  sub: { level: 0.07, octave: -1 }, filter: { cutoff: 5600, resonance: 0.04 },
  filterEnvelope: { amount: 7, attack: 0.005, decay: 0.2, sustain: 0.2, release: 0.2 },
  amp: { attack: 0.01, decay: 0.2, sustain: 0.75, release: 0.19 },
  lfo: { shape: "sine", rateHz: 4.7, pitch: 0.045, positionA: 0.07 } });
export const motionBass = () => wavetable({ oscA: { wave: "saw", morphTo: "square", position: 0.4, unison: 3, detune: 10, spread: 0.45, phaseSpread: 0.33 },
  oscB: { wave: "organ", morphTo: "glass", position: 0.3, pitch: 12 }, mix: 0.28,
  voiceMode: "mono", glide: 0.018, filter: { cutoff: 1700, resonance: 0.15 },
  amp: { attack: 0.007, decay: 0.2, sustain: 0.68, release: 0.07 },
  lfo: { shape: "triangle", rateHz: 140 / 60 * 2, phase: 0.25, cutoff: 16, positionA: 0.28, positionB: -0.18, level: 0.2 } });
export const skyPad = () => wavetable({ oscA: { wave: "triangle", morphTo: "saw", position: 0.35, unison: 4, detune: 17, spread: 1, phaseSpread: 0.7 },
  oscB: { wave: "glass", pitch: 12 }, mix: 0.07, noise: { level: 0.016 },
  filter: { cutoff: 1800, resonance: 0.06 }, amp: { attack: 0.5, decay: 0.2, sustain: 0.65, release: 0.7 },
  lfo: { shape: "sine", rateHz: 140 / 60 / 8, positionA: 0.25, cutoff: 4 } });
