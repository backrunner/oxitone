import type { InstrumentRef } from "@oxitone/protocol";

/** Short punch kick, snappy snare and metallic tops. Shared by native/Wasm. */
export const electronicKit = (): InstrumentRef => ({
  pluginId: "example.drums", pluginVersion: "1.0.0", parameters: {
    volume: 1, decay: 1, kickTune: 51.913, kickSweep: 310, kickClick: 0.7,
    kickDecay: 0.52, snareTune: 185, snareSnap: 0.95, snareDecay: 1.05,
    closedDecay: 0.48, openDecay: 0.85, hatTone: 0.72,
  },
});
