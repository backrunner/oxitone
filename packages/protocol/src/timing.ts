import { z } from "zod";
import { beatWireSchema } from "./beat.js";

/** Accompanies a versioned ProjectSnapshot; evaluated without an engine or assets. */
export const beatDurationQuerySchema = z.object({
  startBeat: beatWireSchema,
  durationSeconds: z.number().finite().nonnegative(),
});
export type BeatDurationQuery = z.infer<typeof beatDurationQuerySchema>;
export const beatDurationResultSchema = z.object({
  protocolVersion: z.string(),
  durationBeats: beatWireSchema,
});
