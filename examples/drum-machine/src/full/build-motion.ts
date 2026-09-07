import type { createDrumMix } from "./drum-mix.js";
import { automation } from "./shared.js";

const hz = (value: number) => Math.log(value / 20) / Math.log(1000);
export function applyBuildMotion(d: ReturnType<typeof createDrumMix>) {
  const ramp = (values: readonly [number, number][]) => automation.polyline([
    { beat: 0, value: values[0]![1] },
    ...[32, 224].flatMap(start => values.map(([offset, value]) => ({ beat: start + offset, value }))),
  ]);
  d.roll.automate("snareTune", ramp([[0, (165 - 100) / 220], [32, (185 - 100) / 220],
    [48, (240 - 100) / 220], [60, (300 - 100) / 220], [64, (165 - 100) / 220]]));
  d.roll.automate("snareDecay", ramp([[0, (0.95 - 0.25) / 1.75], [48, (0.65 - 0.25) / 1.75],
    [62, (0.38 - 0.25) / 1.75], [64, (0.95 - 0.25) / 1.75]]));
  d.buildBus.automate("insert.0.parameter.cutoffHz", ramp([[0, hz(130)], [48, hz(190)],
    [62.8, hz(620)], [64, hz(130)]]));
  d.buildBus.automate("level", ramp([[0, 0.4], [32, 0.46], [56, 0.5], [62.95, 0.5],
    [63.08, 0], [63.98, 0], [64, 0.5]]));
  d.riser.automate("oscA.pitch", ramp([[0, 0.25], [32, 0.25], [62.9, 0.75], [64, 0.25]]));
  d.riser.automate("filter.cutoff", ramp([[0, hz(1500)], [32, hz(1500)], [62.9, hz(10500)], [64, hz(1500)]]));
}
