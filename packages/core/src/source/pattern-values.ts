import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { Pattern } from "../patterns/pattern.js";
import type { PatternOptions } from "../patterns/pattern.js";
import { createSource } from "./graph.js";
import type { SourceValue } from "./types.js";
import { noteSourceInput, sourceNoteInput } from "./notes.js";

const sources = new WeakMap<Pattern, SourceValue>();

export function patternSourceOf(pattern: Pattern): SourceValue {
  if (pattern.parts.length)
    throw new OxitoneError(ErrorCode.EditNotRepresentable, "Select a Channel part to edit this Pattern");
  let source = sources.get(pattern);
  if (!source) {
    source = createSource({
      kind: "literal",
      lengthBeats: pattern.lengthBeats,
      notes: pattern.notes.map(noteSourceInput),
    });
    sources.set(pattern, source);
  }
  return source;
}

export function patternFromSource(source: SourceValue, options: Pick<PatternOptions, "id" | "name"> = {}): Pattern {
  const pattern = new Pattern({
    ...options,
    lengthBeats: source.lengthBeats,
    notes: source.events.map((event) => sourceNoteInput(event.note)),
  });
  sources.set(pattern, source);
  return pattern;
}
