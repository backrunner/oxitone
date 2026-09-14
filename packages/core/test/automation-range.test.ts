import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { createAutomationNamespace, AutomationSource } from "../src/index.js";
const a = createAutomationNamespace();

it("supports range overlays on every automation builder and keeps the base immutable", () => {
  const constant = a.constant(0.25),
    second = a.constant(0.8),
    wave = { periodBeats: 4 };
  const sources = [
    constant,
    a.curve([{ beat: 0, value: 0.5 }]),
    a.polyline([{ beat: 0, value: 0.5 }]),
    a.line(0, 1, 4),
    a.gate({ periodBeats: 1, duty: 0.5 }),
    a.chance({ probability: 0.5, seed: 3, rate: 4 }),
    a.wave("sine", wave),
    a.sine(wave),
    a.cos(wave),
    a.triangle(wave),
    a.saw(wave),
    a.ramp(wave),
    a.square(wave),
    a.map(constant, { min: 0.1, max: 0.8 }),
    a.clamp(constant),
    a.invert(constant),
    a.quantize(constant, 4),
    a.scale(constant, 2),
    a.offset(constant, 0.2),
    a.mix(constant, second),
    a.add(constant, second),
    a.multiply(constant, second),
    a.min(constant, second),
    a.max(constant, second),
  ];
  expect(sources).toHaveLength(24);
  for (const source of sources) {
    const before = source.toSpec();
    const result = source.replaceRange({ start: 2, end: 4 }, second).toSpec();
    expect(result).toMatchObject({ kind: "replaceRange", base: before, replacement: second.toSpec() });
    expect(source.toSpec()).toEqual(before);
  }
});

it("matches shared golden sources, reduces repeated hard edits, and retains fade layering", () => {
  const fixture = JSON.parse(
    readFileSync(new URL("../../../schemas/fixtures/automation-range.json", import.meta.url), "utf8"),
  );
  const base = a.constant(0.25),
    replacement = a.line(0.2, 0.8, 1);
  expect(base.replaceRange({ start: 2, end: 4 }, replacement).toSpec()).toEqual(fixture.cases[0].source);
  expect(base.replaceRange({ start: 2, end: 4, fadeBeats: 0.5 }, replacement).toSpec()).toEqual(
    fixture.cases[1].source,
  );
  expect(
    base
      .replaceRange({ start: 2, end: 4 }, [
        { beat: 0, value: 0.2, curve: { kind: "bezier", out: [1 / 3, 0.6], in: [2 / 3, 0.9] } },
        { beat: 1, value: 0.8, curve: { kind: "step" } },
        { beat: 2, value: 0.4 },
      ])
      .toSpec(),
  ).toEqual(fixture.cases[2].source);
  expect(() =>
    base.replaceRange({ start: 2, end: 4 }, [
      { beat: 0, value: 0.2, curve: { kind: "bezier", out: [-1, 0.6], in: [2 / 3, 0.9] } },
      { beat: 1, value: 0.8 },
    ]),
  ).toThrowError(expect.objectContaining({ code: "AutomationRange" }));
  let edited = base;
  for (let i = 0; i < 300; i++) edited = edited.replaceRange({ start: 2, end: 4 }, a.constant(i % 2));
  expect(edited.toSpec()).toMatchObject({ kind: "replaceRange", base: base.toSpec() });
  expect(edited.replaceRange({ start: 2, end: 4, fadeBeats: 0.5 }, replacement).toSpec()).toMatchObject({
    base: edited.toSpec(),
  });
  expect(() => base.replaceRange({ start: 2, end: 2 }, replacement)).toThrowError(
    expect.objectContaining({ code: "AutomationRange" }),
  );
  expect(() => base.replaceRange({ start: 2, end: 3, fadeBeats: 0.6 }, replacement)).toThrowError(
    expect.objectContaining({ code: "AutomationRange" }),
  );
  expect(() => base.replaceRange({ start: NaN, end: 3 }, replacement)).toThrowError(
    expect.objectContaining({ code: "AutomationNonFinite" }),
  );
  const bad = { ...fixture.cases[0].source, endBeat: { numerator: 1, denominator: 1 } };
  expect(() => new AutomationSource(bad)).toThrowError(expect.objectContaining({ code: "AutomationRange" }));
});
