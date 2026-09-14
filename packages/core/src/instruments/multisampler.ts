import {
  multisamplerStateSchema,
  multisamplerOptionsSchema,
  OxitoneError,
  ErrorCode,
  type InstrumentRef,
  type MultisamplerOptions,
} from "@oxitone/protocol";
import { Sample } from "../arrangement/sample.js";
import { sampler } from "./builders.js";
import { parseAuthoring } from "../authoring-validation.js";

export type { MultisamplerOptions } from "@oxitone/protocol";
export interface SampleRegion {
  sample: Sample;
  rootKey: number;
  keyRange: [number, number];
  velocityRange?: [number, number];
  gain?: number;
}
/** Native multisample instrument with disjoint inclusive MIDI key/velocity regions.
 * Gaps are silent; overlapping regions are rejected. Samples remain Project resources.
 */
export function multisampler(regions: readonly SampleRegion[], options: MultisamplerOptions = {}): InstrumentRef {
  if (
    !Array.isArray(regions) ||
    !regions.length ||
    regions.length > 256 ||
    regions.some((r) => !(r?.sample instanceof Sample))
  ) {
    throw new OxitoneError(ErrorCode.InvalidProject, "multisampler requires 1–256 Sample regions");
  }
  const { transpose, ...base } = parseAuthoring(multisamplerOptionsSchema, options, "multisampler");
  const state = parseAuthoring(
    multisamplerStateSchema,
    {
      version: 1,
      regions: regions.map((r, i) => ({
        resource: `region_${i}`,
        rootKey: r.rootKey,
        keyRange: r.keyRange,
        velocityRange: r.velocityRange ?? [1, 127],
        gain: r.gain ?? 1,
      })),
    },
    "multisampler.state",
  );
  const cells = new Uint8Array(128 * 128);
  for (const r of state.regions) {
    const [low, high] = r.keyRange,
      [soft, loud] = r.velocityRange;
    if (low > high || soft > loud) throw new OxitoneError(ErrorCode.InvalidProject, "inverted multisampler region");
    for (let key = low; key <= high; key++)
      for (let v = soft; v <= loud; v++) {
        const index = key * 128 + v;
        if (cells[index])
          throw new OxitoneError(ErrorCode.InvalidProject, "overlapping multisampler key/velocity regions");
        cells[index] = 1;
      }
  }
  const parameters = sampler(regions[0]!.sample, base).parameters;
  if (transpose !== undefined) parameters.transpose = transpose;
  return {
    pluginId: "oxitone.multisampler",
    pluginVersion: "1.0.0",
    parameters,
    state,
    resources: Object.fromEntries(regions.map((r, i) => [`region_${i}`, r.sample.id])),
  };
}
