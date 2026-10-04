import { z } from "zod";
import { patternClipSpecSchema, sampleClipSpecSchema } from "../authoring/specs.js";
import { beatWireSchema } from "../base/beat.js";

export const patternPasteSchema = z
  .object(patternClipSpecSchema.shape)
  .omit({ id: true, patternId: true, trackId: true })
  .strict()
  .refine(
    (c) => c.loopCount === undefined || c.lastBeat === undefined,
    "loopCount and lastBeat are mutually exclusive",
  );
export const samplePasteSchema = sampleClipSpecSchema.omit({ id: true, sampleId: true, trackId: true }).strict();
export const automationPasteSchema = z.strictObject({
  startBeat: beatWireSchema,
  durationBeats: beatWireSchema.optional(),
  enabled: z.boolean().optional(),
});

const index = z.number().int().nonnegative();
/** Ordinary source-level Playlist operations; collection indices refer to builder order. */
export const arrangementSingleEditSchema = z.discriminatedUnion("action", [
  z.strictObject({
    action: z.literal("paste"),
    kind: z.enum(["pattern", "sample", "automation"]),
    resource: index,
    track: index,
    startBeat: z.number().finite().nonnegative(),
    settings: z.union([patternPasteSchema, samplePasteSchema, automationPasteSchema]),
    channels: z.array(index).max(256).optional(),
  }),
  z.object({
    action: z.literal("place"),
    kind: z.enum(["pattern", "sample", "automation"]),
    resource: index,
    track: index,
    startBeat: z.number().finite().nonnegative(),
    durationBeats: z.number().finite().positive().optional(),
    channels: z.array(index).max(256).optional(),
  }),
  z.object({
    action: z.literal("move"),
    kind: z.enum(["pattern", "sample", "automation"]),
    resource: index,
    clip: index,
    track: index,
    startBeat: z.number().finite().nonnegative(),
  }),
  z.object({
    action: z.literal("remove"),
    kind: z.enum(["pattern", "sample", "automation"]),
    resource: index,
    clip: index,
  }),
  z.object({
    action: z.literal("duplicate"),
    kind: z.enum(["pattern", "sample", "automation"]),
    resource: index,
    clip: index,
    track: index,
    startBeat: z.number().finite().nonnegative(),
  }),
  z.object({
    action: z.literal("resize"),
    kind: z.enum(["pattern", "automation"]),
    resource: index,
    clip: index,
    durationBeats: z.number().finite().positive(),
  }),
  z.object({
    action: z.literal("fitSample"),
    kind: z.literal("sample"),
    resource: index,
    clip: index,
    durationBeats: z.number().finite().positive(),
  }),
  z.object({
    action: z.literal("enable"),
    kind: z.enum(["pattern", "sample", "automation"]),
    resource: index,
    clip: index,
    enabled: z.boolean(),
  }),
]);
/** Every clip index in a batch addresses the pre-edit builder order. One batch is one source undo. */
export const arrangementEditSchema = z.union([
  arrangementSingleEditSchema,
  z.strictObject({ action: z.literal("batch"), edits: z.array(arrangementSingleEditSchema).min(1).max(4096) }),
]);
export type ArrangementSingleEdit = z.infer<typeof arrangementSingleEditSchema>;
export type ArrangementEdit = z.infer<typeof arrangementEditSchema>;
