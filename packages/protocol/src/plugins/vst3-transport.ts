import { z } from "zod";

/** Exact context at the first sample of a native VST3 block. Beats are quarter notes. */
export const vst3TransportSchema = z
  .strictObject({
    projectFrame: z
      .number()
      .int()
      .nonnegative()
      .max(Number.MAX_SAFE_INTEGER - 4096),
    continuousFrame: z
      .number()
      .int()
      .nonnegative()
      .max(Number.MAX_SAFE_INTEGER - 4096),
    projectBeat: z.number().finite().nonnegative().max(Number.MAX_SAFE_INTEGER),
    barBeat: z.number().finite().nonnegative().max(Number.MAX_SAFE_INTEGER),
    tempo: z.number().finite().min(20).max(999),
    timeSignature: z.tuple([
      z.number().int().min(1).max(32),
      z.union([z.literal(1), z.literal(2), z.literal(4), z.literal(8), z.literal(16)]),
    ]),
    playing: z.boolean(),
    cycle: z.tuple([z.number().finite().nonnegative(), z.number().finite().nonnegative()]).optional(),
  })
  .refine((position) => position.barBeat <= position.projectBeat, "barBeat must precede projectBeat")
  .refine((position) => !position.cycle || position.cycle[1] > position.cycle[0], "cycle must have positive length");
export type Vst3Transport = z.infer<typeof vst3TransportSchema>;
