import type { PluginUiManifest } from "../plugin-ui.js";
/** Shared TypeScript → JSON → Rust compatibility fixture. */
export const pluginUiFixture: PluginUiManifest = {
  uiVersion: "1.0", pluginId: "oxitone.wavetable", pluginVersion: "1.0.0", title: "Studio Synth",
  size: { width: 680, height: 460 },
  pages: [{ id: "sound", title: "Sound", groups: [{ id: "tone", title: "Tone", columns: 3,
    controls: [
      { kind: "knob", parameter: "filter.cutoff", label: "Cutoff" },
      { kind: "fader", parameter: "osc.mix", label: "A / B blend" },
      { kind: "choice", parameter: "voiceMode", options: [{ value: 0, label: "Poly" }, { value: 1, label: "Mono" }, { value: 2, label: "Legato" }] },
      { kind: "readout", parameter: "oscA.unison", label: "Voices" },
      { kind: "envelope", attack: "amp.attack", decay: "amp.decay", sustain: "amp.sustain", release: "amp.release" },
      { kind: "oscillator", wave: "oscA.wavetable", morphTo: "oscA.morphTo", position: "oscA.position", phase: "oscA.phase", unison: "oscA.unison", detune: "oscA.detune", spread: "oscA.spread",
        bank: "oscA.bank", warpMode: "oscA.warpMode", warp: "oscA.warp", octave: "oscA.octave" },
      { kind: "subOscillator", wave: "sub.wave", octave: "sub.octave", level: "sub.level" },
      { kind: "filterResponse", mode: "filter.type", cutoff: "filter.cutoff", resonance: "filter.resonance" },
      { kind: "lfoCurve", shape: "lfo.shape", rate: "lfo.rateHz", phase: "lfo.phase" },
      { kind: "modulation", routes: [{ label: "Cutoff", amount: "lfo.cutoff" }] },
    ],
  }] }],
};
