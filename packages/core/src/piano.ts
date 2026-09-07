import { ErrorCode, OxitoneError, multisamplerOptionsSchema, type MultisamplerOptions } from "@oxitone/protocol";
import { multisampler, type SampleRegion } from "./multisampler.js";
import { Sample } from "./sample.js";
import { parseAuthoring } from "./authoring-validation.js";

export interface PianoSample { sample: Sample; rootKey: number; gain?: number }
/** Layers are ordered from softly to strongly struck recordings; never synthesized in JS. */
export interface PianoBank { layers: readonly (readonly PianoSample[])[]; keyRange: readonly [number, number] }

function regions(bank: PianoBank, soft: boolean): SampleRegion[] {
  const fail = () => { throw new OxitoneError(ErrorCode.InvalidProject, "piano requires 2–8 aligned velocity layers and a valid key range"); };
  if (!bank || !Array.isArray(bank.layers) || bank.layers.length < 2 || bank.layers.length > 8 ||
      !Array.isArray(bank.keyRange) || bank.keyRange.length !== 2 ||
      bank.keyRange.some(k => !Number.isInteger(k) || k < 0 || k > 127) || bank.keyRange[0] > bank.keyRange[1]) fail();
  if (bank.layers.some(layer => !Array.isArray(layer) || !layer.length || layer.length * bank.layers.length > 256 ||
      layer.some(s => !s || !(s.sample instanceof Sample) ||
        s.gain !== undefined && (!Number.isFinite(s.gain) || s.gain < 0 || s.gain > 4)))) fail();
  const layers = bank.layers.map(layer => [...layer].sort((a, b) => a.rootKey - b.rootKey));
  const keys = layers[0]!.map(s => s.rootKey);
  if (!keys.length || keys.some((k, i) => !Number.isInteger(k) || k < 0 || k > 127 || i > 0 && k === keys[i - 1]) ||
      layers.some(layer => layer.length !== keys.length || layer.some((s, i) => s.rootKey !== keys[i]))) fail();
  // Soft Piano uses the two quietest recorded dynamics across the full playing range.
  const selected = soft ? layers.slice(0, 2) : layers;
  return selected.flatMap((layer, index) => layer.flatMap((s, i) => {
    const low = Math.max(bank.keyRange[0], i ? Math.floor((keys[i - 1]! + s.rootKey) / 2) + 1 : bank.keyRange[0]);
    const high = Math.min(bank.keyRange[1], i + 1 < keys.length ? Math.floor((s.rootKey + keys[i + 1]!) / 2) : bank.keyRange[1]);
    if (low > high) return [];
    return [{ ...s, keyRange: [low, high] as [number, number],
      velocityRange: [Math.floor(index * 127 / selected.length) + 1, Math.floor((index + 1) * 127 / selected.length)] as [number, number] }];
  }));
}

/** Natural recorded grand, nearest-root key zones and all supplied velocity layers. */
export function grandPiano(bank: PianoBank, options: MultisamplerOptions = {}) {
  const parsed = parseAuthoring(multisamplerOptionsSchema, options, "grandPiano");
  return multisampler(regions(bank, false), { velocitySensitivity: 0.7, ...parsed,
    amp: { attack: 0.002, decay: 0, sustain: 1, release: 0.38, ...parsed.amp } });
}
/** Intimate grand interpretation using softer strikes, softened attack and longer release. */
export function softPiano(bank: PianoBank, options: MultisamplerOptions = {}) {
  const parsed = parseAuthoring(multisamplerOptionsSchema, options, "softPiano");
  return multisampler(regions(bank, true), { velocitySensitivity: 0.45, ...parsed,
    amp: { attack: 0.007, decay: 0, sustain: 1, release: 0.75, ...parsed.amp } });
}
