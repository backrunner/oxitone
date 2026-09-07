import type { PluginUiManifest } from "@oxitone/core";

/** Static imports are watched; editing these panels does not rebuild the audio graph. */
export const drumPanel: PluginUiManifest = {
  uiVersion: "1.0", pluginId: "example.drums", pluginVersion: "1.0.0", title: "Circuit Drums",
  size: { width: 440, height: 280 },
  pages: [{ id: "kit", title: "Kit", groups: [{ id: "kit", title: "Kit shaping", columns: 2,
    controls: [{ kind: "knob", parameter: "decay", label: "Decay" },
      { kind: "knob", parameter: "volume", label: "Output" }] }] }],
};

export const gainPanel: PluginUiManifest = {
  uiVersion: "1.0", pluginId: "fixture.gain", pluginVersion: "1.0.0", title: "Reference Gain",
  size: { width: 440, height: 320 },
  pages: [{ id: "gain", title: "Gain", groups: [{ id: "output", title: "Output", columns: 1,
    controls: [{ kind: "fader", parameter: "gain", label: "Gain" }] }] }],
};

export const pianoPanel: PluginUiManifest = {
  uiVersion: "1.0", pluginId: "oxitone.multisampler", pluginVersion: "1.0.0", title: "VSCO Upright",
  size: { width: 520, height: 440 },
  pages: [{ id: "piano", title: "Piano", groups: [
    { id: "touch", title: "Touch", columns: 4, controls: [
      { kind: "knob", parameter: "velocitySensitivity", label: "Dynamics" },
      { kind: "knob", parameter: "transpose", label: "Transpose" },
      { kind: "knob", parameter: "level", label: "Output" },
      { kind: "knob", parameter: "pan", label: "Pan" },
    ] },
    { id: "envelope", title: "Envelope", columns: 4, controls: [
      { kind: "knob", parameter: "amp.attack", label: "Attack" },
      { kind: "knob", parameter: "amp.decay", label: "Decay" },
      { kind: "knob", parameter: "amp.sustain", label: "Sustain" },
      { kind: "knob", parameter: "amp.release", label: "Release" },
    ] },
  ] }],
};
