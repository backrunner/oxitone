import { describe, expect, it } from "vitest";
import { ErrorCode, type NoteEdit, type PatternSourceDocument } from "@oxitone/protocol";
import { arp, chord, Pattern } from "../src/index.js";

const note = { pitch: 60, start: 0, duration: 1, velocity: 0.8 };
const roundTrip = (pattern: Pattern) => Pattern.fromSource(JSON.parse(JSON.stringify(pattern.toSource())));

describe("generated source edits", () => {
  it("retains the chord input and deletes an arp step without retiming or recurving", () => {
    const source = arp(chord(60, "major"), "upDown", 0.25, { velocityCurve: { from: 0.2, to: 0.8 } });
    const edited = source.edit([{ select: { step: 1 }, remove: true }, { select: { step: 3 }, set: { pitch: 65 } }]);
    expect(edited.notes).toEqual([source.notes[0], source.notes[2], { ...source.notes[3], pitch: 65 }]);
    expect(edited.lengthBeats).toBe(source.lengthBeats);
    expect(edited.outputs.map((event) => event.origin)).toEqual([source.outputs[0]?.origin, source.outputs[2]?.origin, source.outputs[3]?.origin]);
    expect(source.notes).toHaveLength(4);
    expect(edited.toSource().nodes.map((node) => node.kind)).toEqual(["chord", "arp", "edit"]);
    expect(roundTrip(edited).outputs).toEqual(edited.outputs);
  });

  it("replays musical rules after upstream root and rate changes", () => {
    const source = arp(chord(60, "major"), "upDown", 0.25).edit([
      { select: { step: 1 }, shift: { pitch: 1 } }, { select: { step: 3 }, set: { pitch: 65 } },
    ]);
    const document = source.toSource();
    for (const node of document.nodes) {
      if (node.kind === "chord") node.root = 62;
      if (node.kind === "arp") node.rate = 0.5;
    }
    const rebuilt = Pattern.fromSource(document);
    expect(rebuilt.notes.map((n) => n.pitch)).toEqual([62, 67, 69, 65]);
    expect(rebuilt.notes.map((n) => n.start)).toEqual([0, 0.5, 1, 1.5]);
  });

  it("distinguishes original degrees from voiced order and clamped duplicates", () => {
    const source = chord(60, "major", { inversion: 1, voicing: "open" });
    expect(source.outputs.map((event) => event.origin)).toEqual([
      { kind: "chord", degree: 3, voice: 0 }, { kind: "chord", degree: 2, voice: 1 }, { kind: "chord", degree: 1, voice: 2 },
    ]);
    expect(source.edit([{ select: { degree: 1 }, set: { velocity: 0.2 } }]).notes[2]?.velocity).toBe(0.2);
    expect(source.edit([{ select: { voice: 0 }, set: { pitch: 54 } }]).notes[0]?.pitch).toBe(54);
    expect(chord(124, "aug").edit([{ select: { degree: 2 }, set: { pitch: 120 } }]).notes.map((n) => n.pitch)).toEqual([124, 120, 127]);
  });

  it("keeps a shared DAG and scopes repeat and concat edits", () => {
    const phrase = arp(chord(60, "minor"), "up", 0.5);
    const song = phrase.concat(phrase.transpose(12)).repeat(3);
    const changed = song.edit([{ select: { iteration: 1, note: { segment: 0, note: { step: 2 } } }, set: { pitch: 70 } }]);
    expect(changed.notes.filter((n) => n.pitch === 70)).toHaveLength(1);
    expect(phrase.notes.map((n) => n.pitch)).toEqual([60, 63, 67]);
    const document = changed.toSource();
    expect(document.nodes.filter((n) => n.kind === "arp")).toHaveLength(1);
    expect(roundTrip(changed).toSource()).toEqual(document);
    expect(roundTrip(changed).outputs).toEqual(changed.outputs);
  });

  it("keeps origins and selectors through moves, transforms and source windows", () => {
    const source = arp([60, 64, 67], "up", 1, { gate: 1 }).repeat(2);
    const moved = source.edit([{ select: { iteration: 0, note: { step: 0 } }, set: { start: 4.5 } }]);
    const again = moved.edit([{ select: { iteration: 0, note: { step: 0 } }, set: { velocity: 0.3 } }]);
    expect(again.notes.find((n) => n.start === 4.5)?.velocity).toBe(0.3);
    const window = source.slice(0.5, 3.5).transpose(12).velocity(0.5);
    expect(window.notes.map((n) => [n.start, n.duration])).toEqual([[0, 0.5], [0.5, 1], [1.5, 1], [2.5, 0.5]]);
    expect(window.outputs.map((event) => event.origin)).toEqual(source.outputs.slice(0, 4).map((event) => event.origin));
    expect(roundTrip(window).outputs).toEqual(window.outputs);
  });

  it("inserts without resizing or altering old notes, and permits later editing", () => {
    const source = arp([60, 64], "up", 1);
    const changed = source.edit([{ insert: { ...note, start: 0.5 } }]);
    expect(changed.lengthBeats).toBe(2);
    expect(changed.notes.filter((n) => n.start !== 0.5)).toEqual(source.notes);
    expect(changed.edit([{ select: { inserted: 0 }, remove: true }]).notes).toEqual(source.notes);
    const two = changed.edit([{ insert: { ...note, start: 1.5 } }]);
    expect(two.edit([{ select: { inserted: 1 }, set: { pitch: 70 } }]).notes.find((n) => n.start === 1.5)?.pitch).toBe(70);
  });

  it("does not persist IDs, and preserves random arp output across unrelated creations", () => {
    const source = arp(chord(60, "major"), "random", 0.25, { seed: 0xffff_ffff_ffff_ffffn });
    for (let i = 0; i < 20; i++) chord(70, "minor");
    expect(roundTrip(source).outputs).toEqual(source.outputs);
    const literal = new Pattern({ id: "pat_explicit", lengthBeats: 1, notes: [{ ...note, id: "note_explicit" }] });
    expect(JSON.stringify(literal.toSource())).not.toContain("explicit");
    expect(JSON.stringify(source.toSource())).not.toMatch(/"id"|pat_/);
  });

  it("freezes source outputs and isolates options, edit input, and serialized copies", () => {
    const options = { velocity: 0.5 };
    const base = chord(60, "major", options);
    const operations: NoteEdit[] = [{ select: { degree: 1 }, set: { tags: ["accent"] } }];
    const edited = base.edit(operations);
    options.velocity = 0.1;
    const op = operations[0];
    if (op && "set" in op) op.set.tags?.push("later");
    expect(edited.notes[0]?.tags).toEqual(["accent"]);
    expect(base.notes[0]?.velocity).toBe(0.5);
    expect(Object.isFrozen(edited.outputs[0]?.origin)).toBe(true);
    expect(Object.isFrozen(edited.outputs[0]?.note.tags)).toBe(true);
    const document = edited.toSource(); document.nodes.length = 0;
    expect(edited.toSource().nodes).toHaveLength(2);
  });

  it("reduces repeated absolute drags to one edit node and one set", () => {
    let source = arp([60, 64, 67], "up", 1);
    for (let i = 0; i < 100; i++) source = source.edit([{ select: { step: 1 }, set: { pitch: 50 + i % 20 } }]);
    expect(source.toSource().nodes).toHaveLength(3);
    expect(source.toSource().nodes.at(-1)).toMatchObject({ kind: "edit", operations: [{ select: { step: 1 }, set: { pitch: 69 } }] });
    expect(source.edit([])).toBe(source);
  });
});

describe("source validation", () => {
  it("preserves sequential writes through degree/voice aliases instead of unsafe reduction", () => {
    const source = chord(60, "major").edit([
      { select: { degree: 1 }, set: { pitch: 61, velocity: 0.1 } },
      { select: { voice: 0 }, set: { pitch: 62, velocity: 0.2 } },
    ]).edit([{ select: { degree: 1 }, set: { pitch: 63 } }]);
    expect(source.notes[0]).toMatchObject({ pitch: 63, velocity: 0.2 });
    expect(roundTrip(source).outputs).toEqual(source.outputs);
    const nested = chord(60, "major").concat(chord(64, "minor")).repeat(2);
    expect(nested.edit([{ select: { iteration: 1, note: { segment: 0, note: { voice: 0 } } }, set: { pitch: 50 } }]).notes.filter((n) => n.pitch === 50)).toHaveLength(1);
  });

  it("treats undefined optional edit fields as omitted, including during reduction", () => {
    const edited = chord(60, "major").edit([{ select: { degree: 1 }, set: { pitch: 65, chance: 0.5 } }])
      .edit([{ select: { degree: 1 }, set: { pitch: undefined, chance: undefined } }]);
    expect(edited.notes[0]).toMatchObject({ pitch: 65, chance: 0.5 });
    expect(roundTrip(edited).outputs).toEqual(edited.outputs);
  });

  it("rejects absent, ambiguous and conflicting edits without changing the source", () => {
    const source = new Pattern({ lengthBeats: 1, notes: [note, note] });
    expect(() => source.edit([{ select: { at: { start: 0, pitch: 60 } }, remove: true }])).toThrowError(expect.objectContaining({ code: ErrorCode.EditTargetAmbiguous }));
    expect(source.edit([{ select: { at: { start: 0, pitch: 60 }, occurrence: 1 }, remove: true }]).notes).toHaveLength(1);
    const phrase = arp([60], "up", 1);
    expect(() => phrase.edit([{ select: { step: 1 }, remove: true }])).toThrowError(expect.objectContaining({ code: ErrorCode.EditTargetMissing }));
    expect(() => phrase.edit([{ select: { step: 0 }, set: { pitch: 61 } }, { select: { step: 0 }, remove: true }])).toThrowError(expect.objectContaining({ code: ErrorCode.EditScopeConflict }));
    expect(phrase.edit([{ select: { step: 0 }, remove: true }, { select: { step: 0 }, remove: true }]).notes).toEqual([]);
    expect(() => phrase.edit([{ select: { step: 0 }, expect: { pitch: 61 }, set: { pitch: 62 } }])).toThrowError(expect.objectContaining({ code: ErrorCode.SourceChanged }));
    expect(() => phrase.edit([{ select: { step: 0 }, shift: { pitch: -100 } }])).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    expect(phrase.notes[0]?.pitch).toBe(60);
  });

  it("checks all expectations against original base values", () => {
    const source = arp([60], "up", 1).edit([
      { select: { step: 0 }, expect: { pitch: 60 }, set: { pitch: 61 } },
      { select: { step: 0 }, expect: { pitch: 60 }, set: { velocity: 0.2 } },
    ]);
    expect(source.notes[0]).toMatchObject({ pitch: 61, velocity: 0.2 });
  });

  it("rejects unknown formats, fields, cycles, forward references and unreachable nodes", () => {
    const document = chord(60, "major").toSource();
    expect(() => Pattern.fromSource({ ...document, formatVersion: 2 })).toThrowError(expect.objectContaining({ code: ErrorCode.ProtocolVersionUnsupported }));
    expect(() => Pattern.fromSource({ ...document, id: "hidden" })).toThrow();
    expect(() => Pattern.fromSource({ ...document, nodes: [{ kind: "repeat", input: 0, count: 2 }] })).toThrow();
    expect(() => Pattern.fromSource({ ...document, nodes: [...document.nodes, ...document.nodes], root: 1 })).toThrow();
    expect(() => Pattern.fromSource({ ...document, root: 99 })).toThrow();
  });

  it("bounds expansion, source depth and recursive selectors", () => {
    expect(() => chord(60, "major").repeat(100_000)).toThrowError(expect.objectContaining({ code: ErrorCode.BudgetExceeded }));
    let source = chord(60, "major");
    for (let i = 1; i < 64; i++) source = source.transpose(0);
    expect(() => source.transpose(0)).toThrowError(expect.objectContaining({ code: ErrorCode.BudgetExceeded }));
    let deep = chord(60, "major");
    for (let i = 1; i < 63; i++) deep = deep.transpose(0);
    deep = deep.edit([{ select: { degree: 1 }, set: { pitch: 61 } }]);
    expect(deep.edit([{ select: { degree: 1 }, set: { pitch: 62 } }]).notes[0]?.pitch).toBe(62);
    const cycle: { iteration: number; note?: unknown } = { iteration: 0 }; cycle.note = cycle;
    const document: PatternSourceDocument = chord(60, "major").toSource();
    expect(() => Pattern.fromSource({ ...document, nodes: [...document.nodes, { kind: "edit", input: 0, operations: [{ select: cycle, remove: true }] }], root: 1 })).toThrowError(expect.objectContaining({ code: ErrorCode.BudgetExceeded }));
    expect(() => arp([60], "random", 1, { seed: Number.MAX_SAFE_INTEGER + 1 })).toThrow();
    expect(() => chord(60, "major").slice(1, 0)).toThrow();
  });
});
