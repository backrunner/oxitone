import { Pattern } from "@oxitone/core";
import type { NoteEdit, PatternSourceDocument } from "@oxitone/protocol";

/** Only timing edits grow the phrase. Deletion and expression edits keep its rests. */
export function noteEditExtent(source: PatternSourceDocument, operations: readonly NoteEdit[]): number | undefined {
  if (
    !operations.some(
      (op) =>
        "insert" in op ||
        ("set" in op && (op.set.start !== undefined || op.set.duration !== undefined)) ||
        ("shift" in op && (op.shift.start !== undefined || op.shift.duration !== undefined)),
    )
  )
    return;
  const base = Pattern.fromSource(source);
  const edited = base.edit(operations);
  // Unchanged pre-existing tails must not grow the loop during a velocity/pitch edit.
  const previous = new Map(base.outputs.map((output) => [JSON.stringify(output.select), output.note]));
  let end = base.lengthBeats;
  for (const output of edited.outputs) {
    const before = previous.get(JSON.stringify(output.select));
    if (!before || before.start !== output.note.start || before.duration !== output.note.duration) {
      end = Math.max(end, output.note.start + output.note.duration);
    }
  }
  return end > base.lengthBeats ? Math.ceil(end) : undefined;
}
