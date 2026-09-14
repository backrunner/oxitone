import { ErrorCode, OxitoneError, type Beat, type EntityId, type Pitch } from "@oxitone/protocol";
import type { NoteInput } from "./note.js";
import { Pattern } from "../patterns/pattern.js";
import { createSource } from "../source/graph.js";
import { patternFromSource, patternSourceOf } from "../source/pattern-values.js";

export type ArpOrder = "up" | "down" | "upDown" | "random";
export interface ArpVelocityCurve {
  from: number;
  to: number;
}
export interface ArpOptions {
  /** pcg32-v1, unsigned 64-bit musical seed; defaults to 0. */
  seed?: number | bigint;
  gate?: number;
  octaves?: number;
  velocity?: number;
  velocityCurve?: ArpVelocityCurve;
  lengthBeats?: Beat;
  name?: string;
  id?: EntityId;
}

/** Generate rhythm from input pitches while retaining the source and step coordinates. */
export function arp(
  notes: Pattern | readonly (Pitch | NoteInput)[],
  order: ArpOrder,
  rate: Beat,
  options: ArpOptions = {},
): Pattern {
  const { seed, id, name, ...music } = options;
  if (
    seed !== undefined &&
    ((typeof seed === "number" && !Number.isSafeInteger(seed)) || seed < 0 || BigInt(seed) > 0xffff_ffff_ffff_ffffn)
  ) {
    throw new OxitoneError(ErrorCode.InvalidProject, "arp seed must be a safe integer or u64 bigint");
  }
  const input =
    notes instanceof Pattern
      ? patternSourceOf(notes)
      : createSource({
          kind: "literal",
          lengthBeats: 1,
          notes: notes.map((note) => ({
            pitch: typeof note === "number" ? note : note.pitch,
            start: 0,
            duration: 1,
            velocity: 1,
          })),
        });
  return patternFromSource(
    createSource(
      {
        kind: "arp",
        input: 0,
        order,
        rate,
        options: { ...music, ...(seed === undefined ? {} : { seed: String(seed) }) },
      },
      [input],
    ),
    { ...(id === undefined ? {} : { id }), ...(name === undefined ? {} : { name }) },
  );
}
