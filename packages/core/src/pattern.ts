import {
  beatToWire,
  beatFromWire,
  patternSpecSchema,
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
import { parseAuthoring } from "./authoring-validation.js";

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
  private noteList: readonly Readonly<NoteInput>[];
  private restoredSpec?: PatternSpec;
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
    this.noteList = Object.freeze(sortNotes(options.notes ?? []).map(freezeNote));
    if (options.name !== undefined) {
      this.patternName = options.name;
    }
    if (options.id !== undefined) defaultIdGenerator.reserve(this.id);
  }

  get name(): string | undefined {
    return this.patternName;
  }

  get notes(): readonly Readonly<NoteInput>[] { return this.noteList; }

  /** Restore immutable notes in wire order: note ordinal participates in seeded playback. */
  static fromSpec(input: PatternSpec): Pattern {
    const spec = parseAuthoring(patternSpecSchema, input, "pattern");
    const notes = spec.notes.map((note) => {
      const value: NoteInput = { pitch: note.pitch, start: beatFromWire(note.start), duration: beatFromWire(note.duration), velocity: note.velocity };
      if (note.id !== undefined) value.id = note.id;
      if (note.offVelocity !== undefined) value.offVelocity = note.offVelocity;
      if (note.chance !== undefined) value.chance = note.chance;
      if (note.voice !== undefined) value.voice = note.voice;
      if (note.tags !== undefined) value.tags = [...note.tags];
      return value;
    });
    const pattern = new Pattern({ id: spec.id, lengthBeats: beatFromWire(spec.lengthBeats), notes,
      ...(spec.name === undefined ? {} : { name: spec.name }) });
    pattern.noteList = Object.freeze(notes.map(freezeNote));
    pattern.restoredSpec = spec;
    return pattern;
  }

  /** Wire form with canonicalized beats. */
  toSpec(): PatternSpec {
    if (this.restoredSpec !== undefined) return structuredClone(this.restoredSpec);
    const notes: NoteSpec[] = this.notes.map((note) => {
      const spec: NoteSpec = {
        pitch: note.pitch,
        start: beatToWire(note.start),
        duration: beatToWire(note.duration),
        velocity: note.velocity,
      };
      if (note.id !== undefined) spec.id = note.id;
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
