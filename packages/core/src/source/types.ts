import type { NoteSelector, PatternSourceNode, SourceNote } from "@oxitone/protocol";

export type ReadonlySourceNote = Readonly<Omit<SourceNote, "tags">> & { readonly tags?: readonly string[] | undefined };
type ReadonlySelector<T> = { readonly [K in keyof T]: T[K] extends object ? ReadonlySelector<T[K]> : T[K] };

/** Musical generation coordinates. These do not contain wire IDs or source locations. */
export type NoteOrigin =
  | {
      readonly kind: "literal";
      readonly start: number;
      readonly pitch: number;
      readonly voice?: number;
      readonly occurrence: number;
    }
  | { readonly kind: "chord"; readonly degree: number; readonly voice: number }
  | {
      readonly kind: "arp";
      readonly step: number;
      readonly input: NoteOrigin;
      readonly inputOccurrence: number;
      readonly cycleStep: number;
      readonly octave: number;
    }
  | { readonly kind: "repeat"; readonly iteration: number; readonly input: NoteOrigin }
  | { readonly kind: "insert"; readonly note: ReadonlySourceNote; readonly occurrence: number };

export interface SourceEvent {
  readonly note: ReadonlySourceNote;
  readonly select: ReadonlySelector<NoteSelector>;
  readonly origin: NoteOrigin;
}

/** Internal immutable DAG. Node child references are local indexes into inputs. */
export interface SourceValue {
  readonly node: PatternSourceNode;
  readonly inputs: readonly SourceValue[];
  readonly events: readonly SourceEvent[];
  readonly lengthBeats: number;
  readonly depth: number;
}

export interface SourceOutput {
  events: SourceEvent[];
  lengthBeats: number;
}

export function freezeSource<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freezeSource(child);
    Object.freeze(value);
  }
  return value;
}
