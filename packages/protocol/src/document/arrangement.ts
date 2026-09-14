import { z } from "zod";

const index = z.number().int().nonnegative();
/** Ordinary source-level Playlist operations; collection indices refer to builder order. */
export const arrangementEditSchema = z.discriminatedUnion("action", [
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
export type ArrangementEdit = z.infer<typeof arrangementEditSchema>;
