import {
  beatToWire,
  ErrorCode,
  ID_PREFIXES,
  OxitoneError,
  type Beat,
  type EntityId,
  type NoteSpec,
  type PatternSpec,
} from "@oxitone/protocol";
import { defaultIdGenerator } from "./ids.js";
import { freezeNote, sortNotes, type NoteInput } from "./note.js";

/** Construction options for an immutable {@link Pattern}. */
export interface PatternOptions {
  id?: EntityId;
  name?: string;
  lengthBeats: Beat;
  notes?: readonly NoteInput[];
}

/**
 * Immutable note fragment. Notes are validated, stably sorted, and frozen at
 * construction; a pattern never changes after creation.
 */
export class Pattern {
  readonly id: EntityId;
  readonly lengthBeats: number;
  readonly notes: readonly Readonly<NoteInput>[];
  private readonly patternName?: string;

  constructor(options: PatternOptions) {
    if (!Number.isFinite(options.lengthBeats) || options.lengthBeats <= 0) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `pattern lengthBeats must be finite and > 0, got ${options.lengthBeats}`,
        { details: { path: "pattern.lengthBeats" } },
      );
    }
    this.id = options.id ?? defaultIdGenerator.next(ID_PREFIXES.pattern);
    this.lengthBeats = options.lengthBeats;
    this.notes = Object.freeze(sortNotes(options.notes ?? []).map(freezeNote));
    if (options.name !== undefined) {
      this.patternName = options.name;
    }
  }

  get name(): string | undefined {
    return this.patternName;
  }

  /** Wire form with canonicalized beats. */
  toSpec(): PatternSpec {
    const notes: NoteSpec[] = this.notes.map((note) => {
      const spec: NoteSpec = {
        pitch: note.pitch,
        start: beatToWire(note.start),
        duration: beatToWire(note.duration),
        velocity: note.velocity,
      };
      if (note.offVelocity !== undefined) {
        spec.offVelocity = note.offVelocity;
      }
      if (note.chance !== undefined) {
        spec.chance = note.chance;
      }
      if (note.voice !== undefined) {
        spec.voice = note.voice;
      }
      if (note.tags !== undefined) {
        spec.tags = [...note.tags];
      }
      return spec;
    });
    const spec: PatternSpec = {
      id: this.id,
      lengthBeats: beatToWire(this.lengthBeats),
      notes,
    };
    if (this.patternName !== undefined) {
      spec.name = this.patternName;
    }
    return spec;
  }
}
