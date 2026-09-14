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
  type NoteEdit,
  type PatternSourceDocument,
} from "@oxitone/protocol";
import { defaultIdGenerator } from "../ids.js";
import { freezeNote, sortNotes, type NoteInput } from "../notes/note.js";
import { parseAuthoring } from "../authoring-validation.js";
import { createSource, restoreSource, serializeSource, validateSourceNode } from "../source/graph.js";
import { patternFromSource, patternSourceOf } from "../source/pattern-values.js";
import { mergeSets, resolveEdits } from "../source/edits.js";
import type { SourceEvent } from "../source/types.js";

/** Construction options for an immutable {@link Pattern}. */
export interface PatternOptions {
  id?: EntityId;
  name?: string;
  lengthBeats: Beat;
  notes?: readonly NoteInput[];
  parts?: readonly PatternPart[];
}

export interface PatternPart {
  readonly channelId: EntityId;
  readonly pattern: Pattern;
}

/**
 * Immutable note fragment. Notes are validated, stably sorted, and frozen at
 * construction; a pattern never changes after creation.
 */
export class Pattern {
  readonly id: EntityId;
  readonly lengthBeats: number;
  readonly parts: readonly PatternPart[];
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
    const parts = options.parts ?? [];
    this.lengthBeats = parts.reduce((length, part) => Math.max(length, part.pattern.lengthBeats), options.lengthBeats);
    if (
      parts.length > 256 ||
      new Set(parts.map((part) => part.channelId)).size !== parts.length ||
      (parts.length > 0 && (options.notes?.length ?? 0) > 0) ||
      parts.some((part) => part.pattern.parts.length > 0)
    ) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "Pattern parts require unique Channels and leaf Patterns, without root notes",
      );
    }
    this.parts = Object.freeze(parts.map((part) => Object.freeze({ ...part })));
    this.noteList = Object.freeze(sortNotes(options.notes ?? []).map(freezeNote));
    if (options.name !== undefined) {
      this.patternName = options.name;
    }
    if (options.id !== undefined) defaultIdGenerator.reserve(this.id);
  }

  get name(): string | undefined {
    return this.patternName;
  }

  get notes(): readonly Readonly<NoteInput>[] {
    return this.noteList;
  }

  /** Resolved notes with immutable musical selectors and generation origins. */
  get outputs(): readonly SourceEvent[] {
    return patternSourceOf(this).events;
  }

  /** Preserve generation rules and shared inputs; contains no entity IDs. */
  toSource(): PatternSourceDocument {
    return serializeSource(patternSourceOf(this), this.name);
  }

  /** Rebuild a validated authoring DAG, without a native engine or editor cache. */
  static fromSource(document: unknown): Pattern {
    const { source, ...options } = restoreSource(document);
    return patternFromSource(source, options);
  }

  /** Edit base outputs atomically. Removal leaves original timing and length intact. */
  edit(operations: readonly NoteEdit[], options: { lengthBeats?: number } = {}): Pattern {
    if (operations.length === 0 && options.lengthBeats === undefined) return this;
    const source = patternSourceOf(this);
    let node = validateSourceNode({
      kind: "edit",
      input: 0,
      operations: [...operations],
      ...(options.lengthBeats === undefined ? {} : { lengthBeats: options.lengthBeats }),
    });
    if (node.kind !== "edit") throw new OxitoneError(ErrorCode.InvalidProject, "expected edit source");
    let input = source;
    if (source.node.kind === "edit") {
      const merged = mergeSets(source.node.operations, node.operations);
      const base = source.inputs[0];
      if (merged && base) {
        // Resolve against the current revision before reducing. This also works at maximum depth.
        resolveEdits(source.events, node.operations);
        input = base;
        node = {
          kind: "edit",
          input: 0,
          operations: merged,
          ...((node.lengthBeats ?? source.node.lengthBeats) === undefined
            ? {}
            : { lengthBeats: node.lengthBeats ?? source.node.lengthBeats }),
        };
      }
    }
    const edited = createSource(node, [input]);
    return patternFromSource(edited, this.name === undefined ? {} : { name: this.name });
  }

  concat(...patterns: readonly Pattern[]): Pattern {
    const inputs = [this, ...patterns].map(patternSourceOf);
    return patternFromSource(createSource({ kind: "concat", inputs: inputs.map((_, i) => i) }, inputs));
  }

  repeat(count: number): Pattern {
    return patternFromSource(createSource({ kind: "repeat", input: 0, count }, [patternSourceOf(this)]));
  }

  /** Finite note window; crossing notes are clipped, original coordinates are retained. */
  slice(start: number, end: number): Pattern {
    return patternFromSource(createSource({ kind: "slice", input: 0, start, end }, [patternSourceOf(this)]));
  }

  transpose(semitones: number): Pattern {
    return patternFromSource(createSource({ kind: "transpose", input: 0, semitones }, [patternSourceOf(this)]));
  }

  velocity(factor: number): Pattern {
    return patternFromSource(createSource({ kind: "velocity", input: 0, factor }, [patternSourceOf(this)]));
  }

  /** Restore immutable notes in wire order: note ordinal participates in seeded playback. */
  static fromSpec(input: PatternSpec, patterns: ReadonlyMap<string, Pattern> = new Map()): Pattern {
    const spec = parseAuthoring(patternSpecSchema, input, "pattern");
    const notes = spec.notes.map((note) => {
      const value: NoteInput = {
        pitch: note.pitch,
        start: beatFromWire(note.start),
        duration: beatFromWire(note.duration),
        velocity: note.velocity,
      };
      if (note.id !== undefined) value.id = note.id;
      if (note.offVelocity !== undefined) value.offVelocity = note.offVelocity;
      if (note.chance !== undefined) value.chance = note.chance;
      if (note.voice !== undefined) value.voice = note.voice;
      if (note.tags !== undefined) value.tags = [...note.tags];
      return value;
    });
    const pattern = new Pattern({
      id: spec.id,
      lengthBeats: beatFromWire(spec.lengthBeats),
      notes,
      ...(spec.name === undefined ? {} : { name: spec.name }),
      ...(spec.parts === undefined
        ? {}
        : {
            parts: spec.parts.map((part) => {
              const pattern = patterns.get(part.patternId);
              if (!pattern) throw new OxitoneError(ErrorCode.InvalidProject, "missing Pattern part");
              return { channelId: part.channelId, pattern };
            }),
          }),
    });
    if (pattern.lengthBeats !== beatFromWire(spec.lengthBeats))
      throw new OxitoneError(ErrorCode.InvalidProject, "Pattern part exceeds its root length");
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
    if (this.parts.length > 0)
      spec.parts = this.parts.map((part) => ({ channelId: part.channelId, patternId: part.pattern.id }));
    return spec;
  }
}
