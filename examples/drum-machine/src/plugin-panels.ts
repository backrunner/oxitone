import type { PluginUiManifest } from "@oxitone/core";

/** Static imports are watched; editing these panels does not rebuild the audio graph. */
export const drumPanel: PluginUiManifest = {
  uiVersion: "1.0", pluginId: "example.drums", pluginVersion: "1.0.0", title: "Circuit Drums",
  size: { width: 620, height: 460 },
  pages: [{ id: "kit", title: "Kit", groups: [{ id: "kit", title: "Kit shaping", columns: 4,
    controls: [{ kind: "knob", parameter: "decay", label: "Decay" },
      { kind: "knob", parameter: "volume", label: "Output" },
      ...(["kickTune", "kickSweep", "kickClick", "kickDecay", "snareTune", "snareSnap", "snareDecay", "hatTone", "closedDecay", "openDecay"] as const)
        .map(parameter => ({ kind: "knob" as const, parameter }))] }] }],
};

export const gainPanel: PluginUiManifest = {
  uiVersion: "1.0", pluginId: "fixture.gain", pluginVersion: "1.0.0", title: "Reference Gain",
  size: { width: 440, height: 320 },
  pages: [{ id: "gain", title: "Gain", groups: [{ id: "output", title: "Output", columns: 1,
    controls: [{ kind: "fader", parameter: "gain", label: "Gain" }] }] }],
};
