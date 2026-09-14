import { z } from "zod";
import { effectRefSchema, instrumentRefSchema } from "../authoring/refs.js";

const index = z.number().int().nonnegative();
const mix = z
  .object({
    level: z.number().finite().min(0).max(2).optional(),
    pan: z.number().finite().min(-1).max(1).optional(),
    mute: z.boolean().optional(),
    solo: z.boolean().optional(),
  })
  .strict();
/** Source-level operations address builder order; execution IDs never enter authored code. */
export const projectEditSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("channel"), index, values: mix }).strict(),
  z.object({ kind: z.literal("bus"), index, values: mix }).strict(),
  z
    .object({
      kind: z.literal("track"),
      index,
      enabled: z.boolean().optional(),
      mute: z.boolean().optional(),
      solo: z.boolean().optional(),
    })
    .strict()
    .refine(
      (edit) => edit.enabled !== undefined || edit.mute !== undefined || edit.solo !== undefined,
      "track edit is empty",
    ),
  z.object({ kind: z.literal("tempo"), bpm: z.number().finite().min(20).max(999) }).strict(),
  z
    .object({
      kind: z.literal("effectOrder"),
      owner: z.enum(["channel", "bus"]),
      index,
      order: z.array(index).max(256),
    })
    .strict(),
  z
    .object({ kind: z.literal("instrument"), index, config: instrumentRefSchema.omit({ instanceId: true }).strict() })
    .strict(),
  z
    .object({
      kind: z.literal("effect"),
      owner: z.enum(["channel", "bus"]),
      index,
      slot: index.optional(),
      config: effectRefSchema.omit({ instanceId: true }).strict().optional(),
    })
    .strict(),
]);
export type ProjectEdit = z.infer<typeof projectEditSchema>;
