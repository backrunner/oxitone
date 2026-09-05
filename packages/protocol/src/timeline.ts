import { z } from "zod";
import { beatWireSchema } from "./beat.js";
import { entityIdSchema, frameWireSchema } from "./primitives.js";

/** Global tempo segment; `curve` describes the transition to the next one. */
export const tempoSegmentSchema = z.object({
  startBeat: beatWireSchema,
  bpm: z.number().finite().min(20).max(999),
  curve: z.enum(["step", "linear", "exponential"]).optional(),
});
export type TempoSegment = z.infer<typeof tempoSegmentSchema>;

/** Time-signature change; only legal on bar boundaries. */
export const timeSignatureSegmentSchema = z.object({
  startBar: z.number().int().min(1),
  numerator: z.number().int().min(1),
  denominator: z.number().int().min(1),
});
export type TimeSignatureSegment = z.infer<typeof timeSignatureSegmentSchema>;

/** Shared loop descriptor; `loopCount`/`lastBeat` semantics live on the clip. */
export const loopSpecSchema = z.object({
  startBeat: beatWireSchema.optional(),
  lengthBeats: beatWireSchema,
  count: z.number().int().min(1).optional(),
  lastBeat: beatWireSchema.optional(),
});
export type LoopSpec = z.infer<typeof loopSpecSchema>;

export const markerSpecSchema = z.object({
  id: entityIdSchema,
  name: z.string().optional(),
  startBeat: beatWireSchema,
});
export type MarkerSpec = z.infer<typeof markerSpecSchema>;

export const fadeSpecSchema = z.object({
  lengthFrames: frameWireSchema,
  curve: z.enum(["linear", "equalPower", "exponential"]).optional(),
});
export type FadeSpec = z.infer<typeof fadeSpecSchema>;
