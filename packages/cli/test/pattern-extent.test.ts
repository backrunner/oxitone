import { expect, it } from "vitest";
import { chord, Pattern } from "@oxitone/core";
import { noteEditExtent } from "../src/source/editing/pattern-extent.js";
import { anchorPatternExpression, writePatternEdit } from "../src/source/editing/pattern-writer.js";

it("grows only changed timing, leaves pre-existing tails and deletion alone", () => {
  const pattern = new Pattern({ lengthBeats: 4, notes: [{ start: 0, pitch: 60, duration: 9, velocity: 0.5 }] });
  const select = pattern.outputs[0]!.select;
  const source = pattern.toSource();
  expect(noteEditExtent(source, [{ select, set: { velocity: 0.9, start: 0, duration: 9 } }])).toBeUndefined();
  expect(noteEditExtent(source, [{ select, set: { start: 8, duration: 0.25 } }])).toBe(9);
  expect(noteEditExtent(source, [{ select, remove: true }])).toBeUndefined();
  expect(noteEditExtent(source, [{ insert: { start: 12, pitch: 67, duration: 1, velocity: 0.7 } }])).toBe(13);
});

it("coalesces literal edit options and retains the extended length through subsequent edits", () => {
  const expression = "chord(60, 'major')";
  const text = `export default ${expression};`;
  let current = writePatternEdit({
    text,
    fileName: "song.ts",
    source: chord(60, "major").toSource(),
    anchor: anchorPatternExpression("song.ts", text, text.indexOf(expression), text.length - 1),
    operations: [{ select: { degree: 1 }, set: { start: 8 } }],
    lengthBeats: 12,
  });
  for (const lengthBeats of [16, 20, 24])
    current = writePatternEdit({
      ...current,
      fileName: "song.ts",
      lengthBeats,
      operations: [{ select: { degree: 1 }, set: { start: lengthBeats - 1 } }],
    });
  current = writePatternEdit({
    ...current,
    fileName: "song.ts",
    operations: [{ select: { degree: 1 }, set: { velocity: 0.2 } }],
  });
  expect(current.text.match(/\.edit\(/g)).toHaveLength(1);
  expect(current.text).toContain("lengthBeats: 24");
  expect(Pattern.fromSource(current.source).lengthBeats).toBe(24);
});
