import {
  beatToWire,
  chanceOptionsToWire,
  ErrorCode,
  OxitoneError,
  type AutomationSourceSpec,
  type BeatWire,
  type ChanceOptions,
  type Curve,
  type CurveKind,
  type WaveKind,
} from "@oxitone/protocol";
import { AutomationSource, checkBeat, checkFinite, checkPositive, checkUnitInterval } from "./source.js";

/** Authoring control point: beat as a number, value in 0..1. */
export interface AutomationPointInput {
  beat: number;
  value: number;
  curve?: Curve;
}

/** Options for `gate` (02-domain-spec.md §函数型 automation). */
export interface GateOptions {
  periodBeats: number;
  duty: number;
  phase?: number;
  on?: number;
  off?: number;
}

/** Options for `wave` and its per-kind aliases. */
export interface WaveOptions {
  periodBeats: number;
  phase?: number;
  min?: number;
  max?: number;
  pulseWidth?: number;
}

/** Automation source factory namespace (04-api-contracts.md). */
export interface AutomationNamespace {
  constant(value: number): AutomationSource;
  curve(points: AutomationPointInput[], interpolation?: CurveKind): AutomationSource;
  polyline(points: AutomationPointInput[]): AutomationSource;
  line(from: number, to: number, durationBeats: number): AutomationSource;
  gate(options: GateOptions): AutomationSource;
  chance(options: ChanceOptions): AutomationSource;
  wave(kind: WaveKind, options: WaveOptions): AutomationSource;
  sine(options: WaveOptions): AutomationSource;
  cos(options: WaveOptions): AutomationSource;
  triangle(options: WaveOptions): AutomationSource;
  saw(options: WaveOptions): AutomationSource;
  ramp(options: WaveOptions): AutomationSource;
  square(options: WaveOptions): AutomationSource;
  map(input: AutomationSource, range: { min: number; max: number }): AutomationSource;
  clamp(input: AutomationSource, range?: { min?: number; max?: number }): AutomationSource;
  invert(input: AutomationSource): AutomationSource;
  quantize(input: AutomationSource, steps: number): AutomationSource;
  scale(input: AutomationSource, factor: number): AutomationSource;
  offset(input: AutomationSource, amount: number): AutomationSource;
  mix(left: AutomationSource, right: AutomationSource, amount?: number): AutomationSource;
  add(left: AutomationSource, right: AutomationSource): AutomationSource;
  multiply(left: AutomationSource, right: AutomationSource): AutomationSource;
  min(left: AutomationSource, right: AutomationSource): AutomationSource;
  max(left: AutomationSource, right: AutomationSource): AutomationSource;
}

function specOf(input: AutomationSource, path: string): AutomationSourceSpec {
  if (!(input instanceof AutomationSource)) {
    throw new OxitoneError(ErrorCode.InvalidProject, "expected an AutomationSource", {
      details: { path },
    });
  }
  return input.toSpec();
}

function chanceSpec(options: ChanceOptions): AutomationSourceSpec {
  const selected = [options.rate, options.frequency, options.intervalBeats].filter((value) => value !== undefined);
  if (selected.length !== 1) {
    throw new OxitoneError(
      ErrorCode.AutomationChanceFrequency,
      "chance requires exactly one of rate/frequency/intervalBeats",
      { details: { path: "$.chance" } },
    );
  }
  const value = selected[0] as number;
  checkFinite(value, "$.chance.rate");
  if (value <= 0) {
    throw new OxitoneError(
      ErrorCode.AutomationChanceFrequency,
      `chance rate/frequency/intervalBeats must be > 0, got ${value}`,
      { details: { path: "$.chance.rate" } },
    );
  }
  checkUnitInterval(options.probability, "$.chance.probability");
  if (!Number.isInteger(options.seed) || options.seed < 0) {
    throw new OxitoneError(ErrorCode.AutomationRange, `chance seed must be an integer >= 0`, {
      details: { path: "$.chance.seed" },
    });
  }
  if (options.smoothBeats !== undefined) {
    checkBeat(options.smoothBeats, "$.chance.smoothBeats");
  }
  return chanceOptionsToWire(options, beatToWire);
}

function waveSpec(kind: WaveKind, options: WaveOptions): AutomationSourceSpec {
  checkBeat(options.phase ?? 0, "$.wave.phase");
  checkPositive(options.periodBeats, "$.wave.periodBeats");
  const spec: AutomationSourceSpec = {
    kind: "wave",
    wave: kind,
    periodBeats: beatToWire(options.periodBeats),
    ...(options.phase !== undefined ? { phase: beatToWire(options.phase) } : {}),
    ...(options.min !== undefined ? { min: options.min } : {}),
    ...(options.max !== undefined ? { max: options.max } : {}),
    ...(options.pulseWidth !== undefined ? { pulseWidth: options.pulseWidth } : {}),
  };
  return spec;
}

interface CurvePointWire {
  beat: BeatWire;
  value: number;
  curve?: Curve;
}

function curvePoints(points: AutomationPointInput[]): CurvePointWire[] {
  return points.map((point) => {
    checkBeat(point.beat, "$.curve.points.beat");
    return {
      beat: beatToWire(point.beat),
      value: point.value,
      ...(point.curve !== undefined ? { curve: point.curve } : {}),
    };
  });
}

/**
 * Create the automation source namespace (07-automation-spec.md). Every
 * builder validates its options and the accumulated depth/node budget at
 * creation time and returns an opaque immutable {@link AutomationSource}.
 */
export function createAutomationNamespace(): AutomationNamespace {
  const wrap = (spec: AutomationSourceSpec): AutomationSource => new AutomationSource(spec);
  const unary = (
    op: "clamp" | "invert" | "quantize" | "scale" | "offset",
    input: AutomationSource,
    extras: { steps?: number; amount?: number; min?: number; max?: number },
  ): AutomationSource => wrap({ kind: "unary", op, input: specOf(input, `$.unary.input`), ...extras });
  const binary = (
    op: "mix" | "add" | "multiply" | "min" | "max",
    left: AutomationSource,
    right: AutomationSource,
    amount?: number,
  ): AutomationSource =>
    wrap({
      kind: "binary",
      op,
      left: specOf(left, "$.binary.left"),
      right: specOf(right, "$.binary.right"),
      ...(amount !== undefined ? { amount } : {}),
    });

  return {
    constant: (value) => {
      checkFinite(value, "$.constant.value");
      return wrap({ kind: "constant", value });
    },
    curve: (points, interpolation = "linear") => wrap({ kind: "curve", interpolation, points: curvePoints(points) }),
    polyline: (points) => wrap({ kind: "curve", interpolation: "linear", points: curvePoints(points) }),
    line: (from, to, durationBeats) => {
      checkPositive(durationBeats, "$.line.durationBeats");
      return wrap({
        kind: "curve",
        interpolation: "linear",
        points: [
          { beat: beatToWire(0), value: from },
          { beat: beatToWire(durationBeats), value: to },
        ],
      });
    },
    gate: (options) => {
      checkBeat(options.phase ?? 0, "$.gate.phase");
      checkPositive(options.periodBeats, "$.gate.periodBeats");
      return wrap({
        kind: "gate",
        periodBeats: beatToWire(options.periodBeats),
        duty: options.duty,
        ...(options.phase !== undefined ? { phase: beatToWire(options.phase) } : {}),
        ...(options.on !== undefined ? { on: options.on } : {}),
        ...(options.off !== undefined ? { off: options.off } : {}),
      });
    },
    chance: (options) => wrap(chanceSpec(options)),
    wave: (kind, options) => wrap(waveSpec(kind, options)),
    sine: (options) => wrap(waveSpec("sine", options)),
    cos: (options) => wrap(waveSpec("cos", options)),
    triangle: (options) => wrap(waveSpec("triangle", options)),
    saw: (options) => wrap(waveSpec("saw", options)),
    ramp: (options) => wrap(waveSpec("ramp", options)),
    square: (options) => wrap(waveSpec("square", options)),
    map: (input, range) => wrap({ kind: "map", input: specOf(input, "$.map.input"), min: range.min, max: range.max }),
    clamp: (input, range = {}) =>
      unary("clamp", input, {
        ...(range.min !== undefined ? { min: range.min } : {}),
        ...(range.max !== undefined ? { max: range.max } : {}),
      }),
    invert: (input) => unary("invert", input, {}),
    quantize: (input, steps) => unary("quantize", input, { steps }),
    scale: (input, factor) => unary("scale", input, { amount: factor }),
    offset: (input, amount) => unary("offset", input, { amount }),
    mix: (left, right, amount) => binary("mix", left, right, amount),
    add: (left, right) => binary("add", left, right),
    multiply: (left, right) => binary("multiply", left, right),
    min: (left, right) => binary("min", left, right),
    max: (left, right) => binary("max", left, right),
  };
}
