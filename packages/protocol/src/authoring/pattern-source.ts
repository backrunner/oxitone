import { z } from "zod";

/** Authoring-only format, independent of the engine snapshot protocol. */
export const PATTERN_SOURCE_FORMAT = 1;
export const PATTERN_SOURCE_LIMITS = { nodes: 4096, depth: 64, events: 100_000 } as const;
const beat = z.number().finite().nonnegative();
const positive = z.number().finite().positive();
const index = z.number().int().nonnegative();
const unit = z.number().finite().min(0).max(1);
const pitch = z.number().int().min(0).max(127);

/** Source notes contain music, never entity IDs. Beats are authoring numbers. */
export const sourceNoteSchema = z.strictObject({
  pitch,
  start: beat,
  duration: positive,
  velocity: unit,
  offVelocity: unit.optional(),
  chance: unit.optional(),
  voice: index.optional(),
  tags: z.array(z.string()).optional(),
});
export type SourceNote = z.infer<typeof sourceNoteSchema>;

export type NoteSelector =
  | { step: number }
  | { degree: number }
  | { voice: number }
  | { at: { start: number; pitch: number; voice?: number | undefined }; occurrence?: number | undefined }
  | { segment: number; note: NoteSelector }
  | { iteration: number; note: NoteSelector }
  | { inserted: number };

export const noteSelectorSchema: z.ZodType<NoteSelector> = z.lazy(() =>
  z.union([
    z.strictObject({ step: index }),
    z.strictObject({ degree: z.number().int().min(1).max(3) }),
    z.strictObject({ voice: index }),
    z.strictObject({
      at: z.strictObject({ start: beat, pitch, voice: index.optional() }),
      occurrence: index.optional(),
    }),
    z.strictObject({ segment: index, note: noteSelectorSchema }),
    z.strictObject({ iteration: index, note: noteSelectorSchema }),
    z.strictObject({ inserted: index }),
  ]),
);

const patch = sourceNoteSchema.partial();
export const noteEditSchema = z.union([
  z.strictObject({ select: noteSelectorSchema, expect: patch.optional(), set: patch }),
  z.strictObject({
    select: noteSelectorSchema,
    expect: patch.optional(),
    shift: z.strictObject({
      pitch: z.number().int().optional(),
      start: z.number().finite().optional(),
      duration: z.number().finite().optional(),
      velocity: z.number().finite().optional(),
    }),
  }),
  z.strictObject({ select: noteSelectorSchema, expect: patch.optional(), remove: z.literal(true) }),
  z.strictObject({ insert: sourceNoteSchema }),
]);
export type NoteEdit = z.infer<typeof noteEditSchema>;

export const chordSourceOptionsSchema = z.strictObject({
  inversion: index.optional(),
  voicing: z.enum(["close", "open"]).optional(),
  duration: positive.optional(),
  velocity: unit.optional(),
  start: beat.optional(),
  lengthBeats: positive.optional(),
});
export const arpSourceOptionsSchema = z.strictObject({
  seed: z
    .string()
    .regex(/^(0|[1-9][0-9]{0,19})$/)
    .optional(),
  gate: positive.max(1).optional(),
  octaves: z.number().int().min(1).max(PATTERN_SOURCE_LIMITS.events).optional(),
  velocity: unit.optional(),
  velocityCurve: z.strictObject({ from: unit, to: unit }).optional(),
  lengthBeats: positive.optional(),
});

/** Children always precede parents; numeric references are document-local, not musical identity. */
export const patternSourceNodeSchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("literal"),
    notes: z.array(sourceNoteSchema).max(PATTERN_SOURCE_LIMITS.events),
    lengthBeats: positive,
  }),
  z.strictObject({
    kind: z.literal("chord"),
    root: pitch,
    quality: z.enum(["major", "minor", "dim", "aug", "sus2", "sus4"]),
    options: chordSourceOptionsSchema,
  }),
  z.strictObject({
    kind: z.literal("arp"),
    input: index,
    order: z.enum(["up", "down", "upDown", "random"]),
    rate: positive,
    options: arpSourceOptionsSchema,
  }),
  z.strictObject({ kind: z.literal("concat"), inputs: z.array(index).min(1).max(PATTERN_SOURCE_LIMITS.nodes) }),
  z.strictObject({
    kind: z.literal("repeat"),
    input: index,
    count: z.number().int().min(1).max(PATTERN_SOURCE_LIMITS.events),
  }),
  z.strictObject({ kind: z.literal("slice"), input: index, start: beat, end: positive }),
  z.strictObject({ kind: z.literal("transpose"), input: index, semitones: z.number().int() }),
  z.strictObject({ kind: z.literal("velocity"), input: index, factor: z.number().finite().min(0).max(2) }),
  z.strictObject({
    kind: z.literal("edit"),
    input: index,
    operations: z.array(noteEditSchema).max(PATTERN_SOURCE_LIMITS.events),
    lengthBeats: positive.optional(),
  }),
]);
export type PatternSourceNode = z.infer<typeof patternSourceNodeSchema>;
export const patternSourceDocumentSchema = z.strictObject({
  formatVersion: z.literal(PATTERN_SOURCE_FORMAT),
  nodes: z.array(patternSourceNodeSchema).min(1).max(PATTERN_SOURCE_LIMITS.nodes),
  root: index,
  name: z.string().optional(),
});
export type PatternSourceDocument = z.infer<typeof patternSourceDocumentSchema>;
