import { expect, it } from "vitest";
import { chord, Pattern, Project } from "../src/index.js";

it("edits an output and phrase length atomically, preserving its generator and origins", () => {
  const base = chord(60, "major", { lengthBeats: 4 }).repeat(2);
  const select = base.outputs[0]!.select;
  const edited = base.edit([{ select, set: { start: 12 } }], { lengthBeats: 16 });
  expect(base.lengthBeats).toBe(8);
  expect(edited.lengthBeats).toBe(16);
  expect(edited.outputs.find(output => JSON.stringify(output.select) === JSON.stringify(select))!.origin).toEqual(base.outputs[0]!.origin);
  expect(Pattern.fromSource(edited.toSource()).lengthBeats).toBe(16);
  const movedAgain = edited.edit([{ select, set: { pitch: 61 } }]);
  expect(movedAgain.lengthBeats).toBe(16);
  expect(movedAgain.toSource().nodes).toHaveLength(edited.toSource().nodes.length);
  expect(edited.repeat(2).notes.some(note => note.start === 28)).toBe(true);
  for (const lengthBeats of [0, -1, NaN, Infinity]) expect(() => base.edit([], { lengthBeats })).toThrow();
});

it("grows a composite root without changing the shorter part's identity or notes", () => {
  const project = new Project();
  const a = project.addChannel(), b = project.addChannel();
  const short = chord(36, "minor", { lengthBeats: 4 });
  const long = chord(60, "major").edit([], { lengthBeats: 12 });
  const phrase = new Pattern({ lengthBeats: 4, parts: [{ channelId: a.id, pattern: long }, { channelId: b.id, pattern: short }] });
  expect(phrase.lengthBeats).toBe(12);
  expect(phrase.parts[1]!.pattern).toBe(short);
  project.addTrack().add(phrase).at({ bar: 1 });
  expect(Project.fromSnapshot(project.snapshot()).snapshot()).toEqual(project.snapshot());
  const malformed = project.snapshot();
  malformed.patterns.find(p => p.parts)!.lengthBeats = { numerator: 4, denominator: 1 };
  expect(() => Project.fromSnapshot(malformed)).toThrow(/exceeds/);
});
