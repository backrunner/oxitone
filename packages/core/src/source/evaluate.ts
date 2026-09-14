import { ErrorCode, OxitoneError, type PatternSourceNode } from "@oxitone/protocol";
import { checkEventBudget, resolveArp, resolveChord } from "./generators.js";
import { resolveEdits } from "./edits.js";
import type { SourceEvent, SourceOutput, SourceValue } from "./types.js";

export function sourceReferences(node: PatternSourceNode): number[] {
  return "input" in node ? [node.input] : node.kind === "concat" ? node.inputs : [];
}

export function withReferences(node: PatternSourceNode, refs: number[]): PatternSourceNode {
  if (node.kind === "concat") return { ...node, inputs: refs };
  if ("input" in node) return { ...node, input: refs[0] ?? 0 };
  return node;
}

export function resolveSource(node: PatternSourceNode, inputs: readonly SourceValue[]): SourceOutput {
  if (node.kind === "literal") {
    const occurrences = new Map<string, number>();
    return {
      lengthBeats: node.lengthBeats,
      events: node.notes.map((note) => {
        const key = JSON.stringify([note.start, note.pitch, note.voice]);
        const occurrence = occurrences.get(key) ?? 0;
        occurrences.set(key, occurrence + 1);
        const at = { start: note.start, pitch: note.pitch, ...(note.voice === undefined ? {} : { voice: note.voice }) };
        return { note, select: { at, occurrence }, origin: { kind: "literal", ...at, occurrence } };
      }),
    };
  }
  if (node.kind === "chord") return resolveChord(node);
  if (node.kind === "concat") {
    checkEventBudget(inputs.reduce((sum, input) => sum + input.events.length, 0));
    let offset = 0;
    const events: SourceEvent[] = [];
    inputs.forEach((input, segment) => {
      for (const event of input.events)
        events.push({
          ...event,
          note: { ...event.note, start: event.note.start + offset },
          select: { segment, note: event.select },
        });
      offset += input.lengthBeats;
    });
    return { events, lengthBeats: offset };
  }
  const input = inputs[0];
  if (!input) throw new OxitoneError(ErrorCode.InvalidProject, "source input missing");
  if (node.kind === "arp") return resolveArp(node, input);
  if (node.kind === "edit")
    return { events: resolveEdits(input.events, node.operations), lengthBeats: node.lengthBeats ?? input.lengthBeats };
  if (node.kind === "repeat") {
    checkEventBudget(input.events.length * node.count);
    const events: SourceEvent[] = [];
    for (let iteration = 0; iteration < node.count; iteration++) {
      for (const event of input.events)
        events.push({
          note: { ...event.note, start: event.note.start + iteration * input.lengthBeats },
          select: { iteration, note: event.select },
          origin: { kind: "repeat", iteration, input: event.origin },
        });
    }
    return { events, lengthBeats: input.lengthBeats * node.count };
  }
  if (node.kind === "slice") {
    if (node.end <= node.start || node.end > input.lengthBeats)
      throw new OxitoneError(ErrorCode.InvalidProject, "slice must be a positive window within the source length");
    const events = input.events.flatMap((event) => {
      const start = Math.max(node.start, event.note.start);
      const end = Math.min(node.end, event.note.start + event.note.duration);
      return end > start
        ? [{ ...event, note: { ...event.note, start: start - node.start, duration: end - start } }]
        : [];
    });
    return { events, lengthBeats: node.end - node.start };
  }
  return {
    lengthBeats: input.lengthBeats,
    events: input.events.map((event) => ({
      ...event,
      note: {
        ...event.note,
        ...(node.kind === "transpose"
          ? { pitch: Math.max(0, Math.min(127, event.note.pitch + node.semitones)) }
          : { velocity: Math.min(1, event.note.velocity * node.factor) }),
      },
    })),
  };
}
