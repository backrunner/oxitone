import {
  automationSourceSchema,
  beatToWire,
  ErrorCode,
  OxitoneError,
  type AutomationSourceSpec,
  type Curve,
  type CurveKind,
} from "@oxitone/protocol";

/** Maximum automation AST depth (07-automation-spec.md §4). */
export const AUTOMATION_MAX_DEPTH = 64;
/** Maximum automation AST node count (07-automation-spec.md §4). */
export const AUTOMATION_MAX_NODES = 256;

/**
 * Opaque authoring value produced by the automation namespace. The wrapped
 * spec is immutable; `toSpec()` re-validates against the protocol schema and
 * returns a deep copy suitable for a snapshot.
 */
export class AutomationSource {
  private readonly spec: AutomationSourceSpec;

  /** @internal Use the automation namespace builders instead. */
  constructor(spec: AutomationSourceSpec) {
    validateAutomationSpec(spec);
    this.spec = structuredClone(spec);
  }

  /** Protocol-validated wire form (deep copy). */
  toSpec(): AutomationSourceSpec {
    return automationSourceSchema.parse(structuredClone(this.spec));
  }

  /** Replace [start,end) in this source's beat domain. Replacement beat zero is start. */
  replaceRange(range: { start: number; end: number; fadeBeats?: number }, replacement: AutomationSource | readonly { beat: number; value: number; curve?: Curve | undefined }[]): AutomationSource {
    if (Array.isArray(replacement)) replacement = new AutomationSource({ kind: "curve", interpolation: "linear",
      points: replacement.map(point => ({ beat: beatToWire(point.beat), value: point.value, ...(point.curve === undefined ? {} : { curve: point.curve }) })) });
    if (!(replacement instanceof AutomationSource)) fail("InvalidProject", "replacement must be an AutomationSource or control points", "range.replacement");
    checkBeat(range.start, "range.start"); checkBeat(range.end, "range.end");
    if (range.end <= range.start) fail("AutomationRange", "replacement interval must have positive length", "range.end");
    const fade = range.fadeBeats ?? 0; checkBeat(fade, "range.fadeBeats");
    if (fade > (range.end - range.start) / 2) fail("AutomationRange", "fade must fit within both ends of the interval", "range.fadeBeats");
    const base = fade === 0 && this.spec.kind === "replaceRange" && this.spec.startBeat.numerator / this.spec.startBeat.denominator === range.start
      && this.spec.endBeat.numerator / this.spec.endBeat.denominator === range.end ? this.spec.base : this.spec;
    return new AutomationSource({ kind: "replaceRange", base, replacement: replacement.toSpec(), startBeat: beatToWire(range.start), endBeat: beatToWire(range.end),
      ...(fade ? { fadeBeats: beatToWire(fade) } : {}) });
  }
}

function fail(code: keyof typeof ErrorCode, message: string, path: string): never {
  throw new OxitoneError(ErrorCode[code], message, { details: { path } });
}

export function checkFinite(value: number, path: string): void {
  if (!Number.isFinite(value)) {
    fail("AutomationNonFinite", `value must be finite, got ${value}`, path);
  }
}

export function checkUnitInterval(value: number, path: string): void {
  checkFinite(value, path);
  if (value < 0 || value > 1) {
    fail("AutomationRange", `value must be in 0..1, got ${value}`, path);
  }
}

/** Authoring beat field: finite, non-negative, converted by the caller. */
export function checkBeat(value: number, path: string): void {
  checkFinite(value, path);
  if (value < 0) {
    fail("AutomationRange", `beat must be >= 0, got ${value}`, path);
  }
}

export function checkPositive(value: number, path: string): void {
  checkFinite(value, path);
  if (value <= 0) {
    fail("AutomationPeriod", `period/duration must be > 0, got ${value}`, path);
  }
}

function checkCurveData(curve: Curve | undefined, path: string): void {
  if (curve === undefined || curve.kind !== "bezier") {
    return;
  }
  for (const [index, point] of [curve.out, curve.in].entries()) {
    checkFinite(point[0], `${path}.${index === 0 ? "out" : "in"}[0]`);
    checkFinite(point[1], `${path}.${index === 0 ? "out" : "in"}[1]`);
    if (point[0] < 0 || point[0] > 1) {
      fail(
        "AutomationRange",
        `bezier control x must be a 0..1 segment fraction, got ${point[0]}`,
        `${path}.${index === 0 ? "out" : "in"}[0]`,
      );
    }
  }
}

function segmentKind(source: CurveKind, pointCurve: Curve | undefined): CurveKind {
  return pointCurve?.kind ?? source;
}

function validateCurve(spec: Extract<AutomationSourceSpec, { kind: "curve" }>, path: string): void {
  if (spec.points.length === 0) {
    fail("AutomationPoints", "curve needs at least one point", `${path}.points`);
  }
  let previous = -1;
  for (const [index, point] of spec.points.entries()) {
    const pointPath = `${path}.points[${index}]`;
    const beat = point.beat.numerator / point.beat.denominator;
    if (beat <= previous) {
      fail("AutomationPoints", "curve point beats must strictly increase", `${pointPath}.beat`);
    }
    previous = beat;
    checkFinite(point.value, `${pointPath}.value`);
    if (point.value < 0 || point.value > 1) {
      fail("AutomationRange", `curve value must be in 0..1, got ${point.value}`, `${pointPath}.value`);
    }
    checkCurveData(point.curve, `${pointPath}.curve`);
  }
  for (let index = 0; index < spec.points.length - 1; index += 1) {
    const from = spec.points[index];
    const to = spec.points[index + 1];
    if (from === undefined || to === undefined) {
      continue;
    }
    if (segmentKind(spec.interpolation, from.curve) === "exponential" && (from.value <= 0 || to.value <= 0)) {
      fail(
        "AutomationExponentialZero",
        "exponential interpolation requires both endpoint values > 0",
        `${path}.points[${index}].value`,
      );
    }
  }
}

function childrenOf(spec: AutomationSourceSpec): AutomationSourceSpec[] {
  switch (spec.kind) {
    case "map":
    case "unary":
      return [spec.input];
    case "binary":
      return [spec.left, spec.right];
    case "replaceRange": return [spec.base, spec.replacement];
    default:
      return [];
  }
}

function validateNode(spec: AutomationSourceSpec, path: string): void {
  switch (spec.kind) {
    case "constant":
      checkFinite(spec.value, `${path}.value`);
      break;
    case "replaceRange": {
      const start = spec.startBeat.numerator / spec.startBeat.denominator, end = spec.endBeat.numerator / spec.endBeat.denominator;
      const fade = spec.fadeBeats ? spec.fadeBeats.numerator / spec.fadeBeats.denominator : 0;
      checkBeat(start, `${path}.startBeat`); checkBeat(end, `${path}.endBeat`); checkBeat(fade, `${path}.fadeBeats`);
      if (end <= start || fade > (end - start) / 2) fail("AutomationRange", "invalid replacement interval or fade", path);
      break;
    }
    case "curve":
      validateCurve(spec, path);
      break;
    case "gate":
      if (spec.periodBeats.numerator === 0) {
        fail("AutomationPeriod", "gate periodBeats must be > 0", `${path}.periodBeats`);
      }
      checkUnitInterval(spec.duty, `${path}.duty`);
      if (spec.on !== undefined) checkUnitInterval(spec.on, `${path}.on`);
      if (spec.off !== undefined) checkUnitInterval(spec.off, `${path}.off`);
      break;
    case "wave":
      if (spec.periodBeats.numerator === 0) {
        fail("AutomationPeriod", "wave periodBeats must be > 0", `${path}.periodBeats`);
      }
      if (spec.min !== undefined) checkUnitInterval(spec.min, `${path}.min`);
      if (spec.max !== undefined) checkUnitInterval(spec.max, `${path}.max`);
      if (spec.pulseWidth !== undefined) checkUnitInterval(spec.pulseWidth, `${path}.pulseWidth`);
      if (spec.min !== undefined && spec.max !== undefined && spec.min > spec.max) {
        fail("AutomationRange", `wave min must be <= max, got ${spec.min} > ${spec.max}`, `${path}.min`);
      }
      break;
    case "chance":
      checkUnitInterval(spec.probability, `${path}.probability`);
      if (spec.rate !== undefined) {
        checkFinite(spec.rate, `${path}.rate`);
        if (spec.rate <= 0) {
          fail("AutomationChanceFrequency", "chance rate must be > 0", `${path}.rate`);
        }
      }
      if (spec.intervalBeats !== undefined && spec.intervalBeats.numerator === 0) {
        fail("AutomationChanceFrequency", "chance intervalBeats must be > 0", `${path}.intervalBeats`);
      }
      break;
    case "map":
      checkUnitInterval(spec.min, `${path}.min`);
      checkUnitInterval(spec.max, `${path}.max`);
      if (spec.min > spec.max) {
        fail("AutomationRange", `map min must be <= max`, `${path}.min`);
      }
      break;
    case "unary":
      if (spec.amount !== undefined) checkFinite(spec.amount, `${path}.amount`);
      if (spec.min !== undefined) checkUnitInterval(spec.min, `${path}.min`);
      if (spec.max !== undefined) checkUnitInterval(spec.max, `${path}.max`);
      if (spec.op === "quantize") {
        if (spec.steps === undefined || !Number.isInteger(spec.steps) || spec.steps < 2) {
          fail("AutomationRange", "quantize steps must be an integer > 1", `${path}.steps`);
        }
      }
      break;
    case "binary":
      if (spec.amount !== undefined) checkUnitInterval(spec.amount, `${path}.amount`);
      break;
  }
}

/**
 * Structural validation of a full source tree (07-automation-spec.md §6):
 * per-node domain checks plus the depth 64 / node 256 budget.
 */
export function validateAutomationSpec(spec: AutomationSourceSpec, path = "$.source"): void {
  let nodes = 0;
  const walk = (current: AutomationSourceSpec, depth: number, currentPath: string): void => {
    if (depth > AUTOMATION_MAX_DEPTH) {
      fail(
        "AutomationDepthLimit",
        `automation AST depth exceeds ${AUTOMATION_MAX_DEPTH}`,
        currentPath,
      );
    }
    nodes += 1;
    if (nodes > AUTOMATION_MAX_NODES) {
      fail(
        "AutomationNodeLimit",
        `automation AST node count exceeds ${AUTOMATION_MAX_NODES}`,
        currentPath,
      );
    }
    validateNode(current, currentPath);
    for (const [index, child] of childrenOf(current).entries()) {
      walk(child, depth + 1, `${currentPath}.${current.kind === "binary" ? (index === 0 ? "left" : "right") : current.kind === "replaceRange" ? (index === 0 ? "base" : "replacement") : "input"}`);
    }
  };
  walk(spec, 1, path);
}
