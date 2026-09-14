import { z } from "zod";
import { beatWireSchema } from "../base/beat.js";
import { automationPointSchema, CURVE_KINDS } from "./curve.js";
import { ErrorCode, OxitoneError } from "../base/errors.js";

type BeatWireValue = z.infer<typeof beatWireSchema>;
type PointValue = z.infer<typeof automationPointSchema>;

/**
 * Wire form of an automation source AST (04-api-contracts.md,
 * 07-automation-spec.md). Recursive by design; the authoring layer must
 * enforce the depth 64 / node 256 budget before compile. Declared manually
 * because zod cannot infer recursive types; optional members include
 * `| undefined` to stay compatible with `exactOptionalPropertyTypes`.
 */
export type AutomationSourceSpec =
  | { kind: "constant"; value: number }
  | { kind: "curve"; interpolation: (typeof CURVE_KINDS)[number]; points: PointValue[] }
  | { kind: "gate"; periodBeats: BeatWireValue; duty: number; phase?: BeatWireValue | undefined; on?: number | undefined; off?: number | undefined }
  | {
      kind: "chance";
      probability: number;
      seed: number;
      smoothBeats?: BeatWireValue | undefined;
      randomPhase?: "absolute" | "restart" | undefined;
      rate: number;
      intervalBeats?: undefined;
    }
  | {
      kind: "chance";
      probability: number;
      seed: number;
      smoothBeats?: BeatWireValue | undefined;
      randomPhase?: "absolute" | "restart" | undefined;
      rate?: undefined;
      intervalBeats: BeatWireValue;
    }
  | { kind: "wave"; wave: WaveKind; periodBeats: BeatWireValue; phase?: BeatWireValue | undefined; min?: number | undefined; max?: number | undefined; pulseWidth?: number | undefined }
  | { kind: "map"; input: AutomationSourceSpec; min: number; max: number }
  | { kind: "unary"; op: "clamp" | "invert" | "quantize" | "scale" | "offset"; input: AutomationSourceSpec; steps?: number | undefined; amount?: number | undefined; min?: number | undefined; max?: number | undefined }
  | { kind: "binary"; op: "mix" | "add" | "multiply" | "min" | "max"; left: AutomationSourceSpec; right: AutomationSourceSpec; amount?: number | undefined }
  | { kind: "replaceRange"; base: AutomationSourceSpec; replacement: AutomationSourceSpec; startBeat: BeatWireValue; endBeat: BeatWireValue; fadeBeats?: BeatWireValue | undefined };

export const WAVE_KINDS = ["sine", "cos", "triangle", "saw", "ramp", "square"] as const;
export type WaveKind = (typeof WAVE_KINDS)[number];

const finite = z.number().finite();
const unitInterval = finite.min(0).max(1);

const chanceCommon = {
  kind: z.literal("chance"),
  probability: unitInterval,
  seed: z.number().int().nonnegative(),
  smoothBeats: beatWireSchema.optional(),
  randomPhase: z.enum(["absolute", "restart"]).optional(),
};

const chanceByRateSchema = z.object({
  ...chanceCommon,
  rate: finite.positive(),
  intervalBeats: z.never().optional(),
});
const chanceByIntervalSchema = z.object({
  ...chanceCommon,
  rate: z.never().optional(),
  intervalBeats: beatWireSchema,
});

/** `chance` wire schema: exactly one of `rate` / `intervalBeats`. */
export const chanceSourceSchema = z.union([chanceByRateSchema, chanceByIntervalSchema]);

export const gateSourceSchema = z.object({
  kind: z.literal("gate"),
  periodBeats: beatWireSchema,
  duty: unitInterval,
  phase: beatWireSchema.optional(),
  on: unitInterval.optional(),
  off: unitInterval.optional(),
});

export const waveSourceSchema = z.object({
  kind: z.literal("wave"),
  wave: z.enum(WAVE_KINDS),
  periodBeats: beatWireSchema,
  phase: beatWireSchema.optional(),
  min: unitInterval.optional(),
  max: unitInterval.optional(),
  pulseWidth: unitInterval.optional(),
});

export const curveSourceSchema = z.object({
  kind: z.literal("curve"),
  interpolation: z.enum(CURVE_KINDS),
  points: z.array(automationPointSchema).min(1),
});

export const constantSourceSchema = z.object({
  kind: z.literal("constant"),
  value: finite,
});

export const automationSourceSchema: z.ZodType<AutomationSourceSpec> = z.lazy(() =>
  z.union([
    constantSourceSchema,
    curveSourceSchema,
    gateSourceSchema,
    chanceSourceSchema,
    waveSourceSchema,
    z.object({ kind: z.literal("replaceRange"), base: automationSourceSchema, replacement: automationSourceSchema,
      startBeat: beatWireSchema, endBeat: beatWireSchema, fadeBeats: beatWireSchema.optional() }),
    z.object({ kind: z.literal("map"), input: automationSourceSchema, min: unitInterval, max: unitInterval }),
    z.object({
      kind: z.literal("unary"),
      op: z.enum(["clamp", "invert", "quantize", "scale", "offset"]),
      input: automationSourceSchema,
      steps: z.number().int().min(2).optional(),
      amount: finite.optional(),
      min: unitInterval.optional(),
      max: unitInterval.optional(),
    }),
    z.object({
      kind: z.literal("binary"),
      op: z.enum(["mix", "add", "multiply", "min", "max"]),
      left: automationSourceSchema,
      right: automationSourceSchema,
      amount: unitInterval.optional(),
    }),
  ]),
);

/** Authoring options for `chance`: exactly one of rate/frequency/intervalBeats. */
export const chanceOptionsSchema = z.union([
  z.strictObject({
    rate: finite.positive(),
    frequency: z.never().optional(),
    intervalBeats: z.never().optional(),
    probability: unitInterval,
    seed: z.number().int().nonnegative(),
    smoothBeats: z.number().finite().nonnegative().optional(),
    randomPhase: z.enum(["absolute", "restart"]).optional(),
  }),
  z.strictObject({
    rate: z.never().optional(),
    frequency: finite.positive(),
    intervalBeats: z.never().optional(),
    probability: unitInterval,
    seed: z.number().int().nonnegative(),
    smoothBeats: z.number().finite().nonnegative().optional(),
    randomPhase: z.enum(["absolute", "restart"]).optional(),
  }),
  z.strictObject({
    rate: z.never().optional(),
    frequency: z.never().optional(),
    intervalBeats: z.number().finite().positive(),
    probability: unitInterval,
    seed: z.number().int().nonnegative(),
    smoothBeats: z.number().finite().nonnegative().optional(),
    randomPhase: z.enum(["absolute", "restart"]).optional(),
  }),
]);
export type ChanceOptions = z.infer<typeof chanceOptionsSchema>;

/**
 * Normalize authoring {@link ChanceOptions} to the wire source. `frequency`
 * is a user-facing alias of `rate` and always serializes as `rate`.
 */
export function chanceOptionsToWire(
  options: ChanceOptions,
  beatToWireFn: (beat: number) => z.infer<typeof beatWireSchema>,
): AutomationSourceSpec {
  const parsed = chanceOptionsSchema.parse(options);
  const base = {
    kind: "chance" as const,
    probability: parsed.probability,
    seed: parsed.seed,
    ...(parsed.smoothBeats !== undefined ? { smoothBeats: beatToWireFn(parsed.smoothBeats) } : {}),
    ...(parsed.randomPhase !== undefined ? { randomPhase: parsed.randomPhase } : {}),
  };
  if (parsed.intervalBeats !== undefined) {
    return { ...base, intervalBeats: beatToWireFn(parsed.intervalBeats) };
  }
  const rate = parsed.rate ?? parsed.frequency;
  if (rate === undefined || !Number.isFinite(rate) || rate <= 0) {
    throw new OxitoneError(
      ErrorCode.AutomationChanceFrequency,
      "chance requires exactly one positive finite rate/frequency/intervalBeats",
    );
  }
  return { ...base, rate };
}
