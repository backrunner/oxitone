import type { InstrumentRef } from "@oxitone/protocol";

/** Synthetic F#-tuned kick, snappy snare and short metallic tops. Shared by native/Wasm. */
export const electronicKit = (): InstrumentRef => ({
  pluginId: "example.drums", pluginVersion: "1.0.0", parameters: {
    volume: 0.94, decay: 1, kickTune: 46.249, kickSweep: 235, kickClick: 0.42,
    kickDecay: 0.8, snareTune: 205, snareSnap: 0.72, snareDecay: 1.15,
    closedDecay: 0.65, openDecay: 0.8, hatTone: 0.6,
  },
});
