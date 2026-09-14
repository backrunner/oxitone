import { describe, expect, it } from "vitest";
import { automationSourceSchema, ErrorCode, OxitoneError, type AutomationSourceSpec } from "@oxitone/protocol";
import { createAutomationNamespace, Project } from "../src/index.js";

const automation = createAutomationNamespace();

function codeOf(fn: () => unknown): string {
  try {
    fn();
  } catch (error) {
    expect(OxitoneError.isOxitoneError(error)).toBe(true);
    return (error as OxitoneError).code;
  }
  throw new Error("expected an OxitoneError");
}

describe("constant / curve / line", () => {
  it("constant serializes and rejects non-finite values", () => {
    expect(automation.constant(0.5).toSpec()).toEqual({ kind: "constant", value: 0.5 });
    expect(codeOf(() => automation.constant(Number.NaN))).toBe(ErrorCode.AutomationNonFinite);
    expect(codeOf(() => automation.constant(Number.POSITIVE_INFINITY))).toBe(ErrorCode.AutomationNonFinite);
  });

  it("curve validates points and serializes beats as wire rationals", () => {
    const source = automation.curve(
      [
        { beat: 0, value: 0.2 },
        { beat: 1.5, value: 0.8, curve: { kind: "smooth" } },
      ],
      "linear",
    );
    expect(source.toSpec()).toEqual({
      kind: "curve",
      interpolation: "linear",
      points: [
        { beat: { numerator: 0, denominator: 1 }, value: 0.2 },
        { beat: { numerator: 3, denominator: 2 }, value: 0.8, curve: { kind: "smooth" } },
      ],
    });
  });

  it("rejects empty, duplicate, and non-increasing points", () => {
    expect(codeOf(() => automation.curve([]))).toBe(ErrorCode.AutomationPoints);
    expect(
      codeOf(() =>
        automation.curve([
          { beat: 1, value: 0 },
          { beat: 1, value: 1 },
        ]),
      ),
    ).toBe(ErrorCode.AutomationPoints);
    expect(
      codeOf(() =>
        automation.curve([
          { beat: 2, value: 0 },
          { beat: 1, value: 1 },
        ]),
      ),
    ).toBe(ErrorCode.AutomationPoints);
  });

  it("rejects out-of-range and non-finite point values and negative beats", () => {
    expect(codeOf(() => automation.curve([{ beat: 0, value: 1.5 }]))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.curve([{ beat: 0, value: Number.NaN }]))).toBe(ErrorCode.AutomationNonFinite);
    expect(codeOf(() => automation.curve([{ beat: -1, value: 0.5 }]))).toBe(ErrorCode.AutomationRange);
  });

  it("rejects exponential segments with non-positive endpoints", () => {
    expect(
      codeOf(() =>
        automation.curve(
          [
            { beat: 0, value: 0 },
            { beat: 1, value: 1 },
          ],
          "exponential",
        ),
      ),
    ).toBe(ErrorCode.AutomationExponentialZero);
    expect(
      codeOf(() =>
        automation.curve([
          { beat: 0, value: 0.5, curve: { kind: "exponential" } },
          { beat: 1, value: 0 },
        ]),
      ),
    ).toBe(ErrorCode.AutomationExponentialZero);
  });

  it("rejects bezier control points outside the segment", () => {
    expect(
      codeOf(() =>
        automation.curve([
          { beat: 0, value: 0, curve: { kind: "bezier", out: [1.5, 0], in: [0.7, 1] } },
          { beat: 2, value: 1 },
        ]),
      ),
    ).toBe(ErrorCode.AutomationRange);
  });

  it("polyline and line build linear curves; line requires positive duration", () => {
    expect(automation.polyline([{ beat: 0, value: 0 }]).toSpec()).toEqual({
      kind: "curve",
      interpolation: "linear",
      points: [{ beat: { numerator: 0, denominator: 1 }, value: 0 }],
    });
    expect(automation.line(0.1, 0.9, 4).toSpec()).toEqual({
      kind: "curve",
      interpolation: "linear",
      points: [
        { beat: { numerator: 0, denominator: 1 }, value: 0.1 },
        { beat: { numerator: 4, denominator: 1 }, value: 0.9 },
      ],
    });
    expect(codeOf(() => automation.line(0, 1, 0))).toBe(ErrorCode.AutomationPeriod);
  });
});

describe("gate and wave", () => {
  it("gate validates period and ranges", () => {
    expect(automation.gate({ periodBeats: 0.5, duty: 0.5 }).toSpec()).toEqual({
      kind: "gate",
      periodBeats: { numerator: 1, denominator: 2 },
      duty: 0.5,
    });
    expect(codeOf(() => automation.gate({ periodBeats: 0, duty: 0.5 }))).toBe(ErrorCode.AutomationPeriod);
    expect(codeOf(() => automation.gate({ periodBeats: 1, duty: 1.2 }))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.gate({ periodBeats: 1, duty: 0.5, on: 2 }))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.gate({ periodBeats: 1, duty: 0.5, phase: -0.5 }))).toBe(ErrorCode.AutomationRange);
  });

  it("wave aliases serialize the kind and validate min/max/pulseWidth", () => {
    expect(automation.sine({ periodBeats: 8, min: 0.2, max: 0.9 }).toSpec()).toEqual({
      kind: "wave",
      wave: "sine",
      periodBeats: { numerator: 8, denominator: 1 },
      min: 0.2,
      max: 0.9,
    });
    expect(automation.square({ periodBeats: 1, pulseWidth: 0.25 }).toSpec()).toMatchObject({
      wave: "square",
      pulseWidth: 0.25,
    });
    expect(automation.cos({ periodBeats: 2 }).toSpec()).toMatchObject({ wave: "cos" });
    expect(automation.triangle({ periodBeats: 2 }).toSpec()).toMatchObject({ wave: "triangle" });
    expect(automation.saw({ periodBeats: 2 }).toSpec()).toMatchObject({ wave: "saw" });
    expect(automation.ramp({ periodBeats: 2 }).toSpec()).toMatchObject({ wave: "ramp" });
    expect(codeOf(() => automation.sine({ periodBeats: -1 }))).toBe(ErrorCode.AutomationPeriod);
    expect(codeOf(() => automation.sine({ periodBeats: 1, min: 0.8, max: 0.2 }))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.square({ periodBeats: 1, pulseWidth: 1.5 }))).toBe(ErrorCode.AutomationRange);
  });
});

describe("chance", () => {
  it("frequency serializes as rate", () => {
    const spec = automation.chance({ frequency: 2, probability: 0.72, seed: 17 }).toSpec();
    expect(spec).toEqual({ kind: "chance", probability: 0.72, seed: 17, rate: 2 });
  });

  it("intervalBeats serializes as a wire beat and smoothBeats/randomPhase pass through", () => {
    const spec = automation
      .chance({ intervalBeats: 0.5, probability: 1, seed: 1, smoothBeats: 0.04, randomPhase: "restart" })
      .toSpec();
    expect(spec).toEqual({
      kind: "chance",
      probability: 1,
      seed: 1,
      intervalBeats: { numerator: 1, denominator: 2 },
      smoothBeats: { numerator: 1, denominator: 25 },
      randomPhase: "restart",
    });
  });

  it("requires exactly one positive finite selector", () => {
    const base = { probability: 0.5, seed: 1 };
    expect(codeOf(() => automation.chance(base as never))).toBe(ErrorCode.AutomationChanceFrequency);
    expect(codeOf(() => automation.chance({ ...base, rate: 2, frequency: 2 } as never))).toBe(
      ErrorCode.AutomationChanceFrequency,
    );
    expect(codeOf(() => automation.chance({ ...base, rate: 0 }))).toBe(ErrorCode.AutomationChanceFrequency);
    expect(codeOf(() => automation.chance({ ...base, rate: Number.NaN }))).toBe(ErrorCode.AutomationNonFinite);
    expect(codeOf(() => automation.chance({ ...base, rate: 1, probability: 1.5 }))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.chance({ ...base, rate: 1, seed: -1 }))).toBe(ErrorCode.AutomationRange);
  });
});

describe("combinators", () => {
  const a = automation.constant(0.25);
  const b = automation.constant(0.75);

  it("serializes unary and binary shapes", () => {
    expect(automation.map(a, { min: 0.2, max: 0.9 }).toSpec()).toEqual({
      kind: "map",
      input: { kind: "constant", value: 0.25 },
      min: 0.2,
      max: 0.9,
    });
    expect(automation.invert(a).toSpec()).toMatchObject({ kind: "unary", op: "invert" });
    expect(automation.clamp(a).toSpec()).toMatchObject({ kind: "unary", op: "clamp" });
    expect(automation.quantize(a, 4).toSpec()).toMatchObject({ op: "quantize", steps: 4 });
    expect(automation.scale(a, 2).toSpec()).toMatchObject({ op: "scale", amount: 2 });
    expect(automation.offset(a, 0.1).toSpec()).toMatchObject({ op: "offset", amount: 0.1 });
    expect(automation.mix(a, b, 0.25).toSpec()).toMatchObject({
      kind: "binary",
      op: "mix",
      amount: 0.25,
    });
    for (const op of ["add", "multiply", "min", "max"] as const) {
      expect(automation[op](a, b).toSpec()).toMatchObject({ kind: "binary", op });
    }
  });

  it("validates combinator options", () => {
    expect(codeOf(() => automation.map(a, { min: -0.1, max: 1 }))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.map(a, { min: 0.9, max: 0.2 }))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.clamp(a, { min: 2 }))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.quantize(a, 1))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.quantize(a, 2.5))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.scale(a, Number.NaN))).toBe(ErrorCode.AutomationNonFinite);
    expect(codeOf(() => automation.mix(a, b, 1.5))).toBe(ErrorCode.AutomationRange);
    expect(codeOf(() => automation.add(a, { nope: true } as never))).toBe(ErrorCode.InvalidProject);
  });

  it("enforces depth 64 and node 256 budgets", () => {
    let deep = automation.constant(0.5);
    for (let index = 0; index < 63; index += 1) {
      deep = automation.invert(deep);
    }
    expect(codeOf(() => automation.invert(deep))).toBe(ErrorCode.AutomationDepthLimit);

    let wide = automation.constant(0.5);
    for (let index = 0; index < 7; index += 1) {
      wide = automation.add(wide, wide);
    }
    expect(automationSourceSchema.parse(wide.toSpec())).toBeTruthy();
    expect(codeOf(() => automation.add(wide, wide))).toBe(ErrorCode.AutomationNodeLimit);
  });
});

describe("lane binding", () => {
  it("addAutomationLane writes snapshot.automation with wire shapes", () => {
    const project = new Project({ seed: 3 });
    const channel = project.addChannel();
    const lane = project.addAutomationLane(
      { entityId: channel.id, parameterId: "level" },
      automation.sine({ periodBeats: 8 }),
      { combine: "add", loop: { startBeat: 4, lengthBeats: 8, count: 2 }, lastBeat: 64 },
    );
    expect(lane.id).toMatch(/^auto_/);
    const snapshot = project.snapshot();
    expect(snapshot.automation).toHaveLength(1);
    expect(snapshot.automation[0]).toEqual({
      id: lane.id,
      target: { entityId: channel.id, parameterId: "level" },
      source: automation.sine({ periodBeats: 8 }).toSpec(),
      combine: "add",
      loop: {
        startBeat: { numerator: 4, denominator: 1 },
        lengthBeats: { numerator: 8, denominator: 1 },
        count: 2,
      },
      lastBeat: { numerator: 64, denominator: 1 },
    });
  });

  it("channel.automate binds the channel entity", () => {
    const project = new Project();
    const channel = project.addChannel();
    const lane = channel.automate("pan", automation.ramp({ periodBeats: 4 }));
    expect(lane.target).toEqual({ entityId: channel.id, parameterId: "pan" });
    expect(project.snapshot().automation[0]?.target.parameterId).toBe("pan");
  });

  it("rejects unknown entities and non-tempo project parameters", () => {
    const project = new Project();
    const source = automation.constant(0.5);
    expect(codeOf(() => project.addAutomationLane({ entityId: "chn_missing", parameterId: "x" }, source))).toBe(
      ErrorCode.AutomationTargetInvalid,
    );
    expect(codeOf(() => project.addAutomationLane({ entityId: project.id, parameterId: "level" }, source))).toBe(
      ErrorCode.AutomationTargetInvalid,
    );
  });

  it("allows exactly one project tempo lane", () => {
    const project = new Project();
    project.addAutomationLane({ entityId: project.id, parameterId: "tempo" }, automation.line(0, 1, 16));
    expect(
      codeOf(() => project.addAutomationLane({ entityId: project.id, parameterId: "tempo" }, automation.constant(0.5))),
    ).toBe(ErrorCode.TempoAutomationConflict);
  });

  it("validates loop options", () => {
    const project = new Project();
    const channel = project.addChannel();
    const source = automation.constant(0.5);
    expect(
      codeOf(() =>
        project.addAutomationLane({ entityId: channel.id, parameterId: "level" }, source, {
          loop: { lengthBeats: 0 },
        }),
      ),
    ).toBe(ErrorCode.InvalidProject);
    expect(
      codeOf(() =>
        project.addAutomationLane({ entityId: channel.id, parameterId: "level" }, source, {
          loop: { lengthBeats: 4, count: 2, lastBeat: 9 },
        }),
      ),
    ).toBe(ErrorCode.InvalidProject);
  });

  it("lane sources round-trip through the protocol schema", () => {
    const nested: AutomationSourceSpec = automation
      .mix(
        automation.map(automation.invert(automation.constant(0.8)), { min: 0.2, max: 0.9 }),
        automation.saw({ periodBeats: 4 }),
        0.25,
      )
      .toSpec();
    expect(automationSourceSchema.parse(nested)).toEqual(nested);
  });
});
