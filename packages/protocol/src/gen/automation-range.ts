import { beatToWire } from "../base/beat.js";
import type { AutomationSourceSpec } from "../authoring/automation-source.js";

const source: AutomationSourceSpec = {
  kind: "replaceRange",
  base: { kind: "constant", value: 0.25 },
  replacement: {
    kind: "curve",
    interpolation: "linear",
    points: [
      { beat: beatToWire(0), value: 0.2 },
      { beat: beatToWire(1), value: 0.8 },
    ],
  },
  startBeat: beatToWire(2),
  endBeat: beatToWire(4),
};
export const automationRangeFixture = {
  version: 1,
  beats: [0, 2, 2.25, 2.5, 3, 3.5, 3.75, 4, 4.25],
  cases: [
    { name: "hard", source, expected: [0.25, 0.2, 0.35, 0.5, 0.8, 0.8, 0.8, 0.25, 0.25] },
    {
      name: "fade",
      source: { ...source, fadeBeats: beatToWire(0.5) },
      expected: [0.25, 0.25, 0.3, 0.5, 0.8, 0.8, 0.525, 0.25, 0.25],
    },
    {
      name: "bezier-and-step-control-points",
      source: {
        ...source,
        replacement: {
          kind: "curve",
          interpolation: "linear",
          points: [
            { beat: beatToWire(0), value: 0.2, curve: { kind: "bezier", out: [1 / 3, 0.6], in: [2 / 3, 0.9] } },
            { beat: beatToWire(1), value: 0.8, curve: { kind: "step" } },
            { beat: beatToWire(2), value: 0.4 },
          ],
        },
      },
      expected: [0.25, 0.2, 0.4765625, 0.6875, 0.8, 0.8, 0.8, 0.25, 0.25],
    },
  ],
};
