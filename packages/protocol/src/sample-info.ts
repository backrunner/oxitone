import { z } from "zod";
import { sampleRefSchema } from "./refs.js";
import { sampleProvenanceSchema } from "./sample-provenance.js";

/** Control-thread inspection request; no engine or project is required. */
export const inspectSampleRequestSchema = z.object({
  protocolVersion: z.string(),
  path: z.string().min(1).refine((path) => !path.includes("\0"), "path must not contain NUL"),
});
export type InspectSampleRequest = z.infer<typeof inspectSampleRequestSchema>;

/** Decoded dimensions plus original-file provenance; never contains PCM. */
export const sampleInfoSchema = sampleRefSchema.pick({
  sha256: true, format: true, sampleRate: true, channels: true, frames: true,
}).extend({
  protocolVersion: z.string(),
  sourceChannels: z.number().int().min(1).max(255),
  sourceBitDepth: z.number().int().positive().optional(),
  decoder: z.string().min(1),
  channelLayoutAction: z.enum(["kept", "downmixed-to-stereo"]),
});
export type SampleInfo = z.infer<typeof sampleInfoSchema>;

/** Explicit import-time WAV cache publication; independent of any engine. */
export const cacheSampleRequestSchema = inspectSampleRequestSchema.extend({
  cacheDir: inspectSampleRequestSchema.shape.path,
});
export type CacheSampleRequest = z.infer<typeof cacheSampleRequestSchema>;

export const cachedSampleInfoSchema = sampleInfoSchema.pick({
  protocolVersion: true, sha256: true, sampleRate: true, channels: true, frames: true,
}).extend({
  path: inspectSampleRequestSchema.shape.path,
  format: z.literal("wav"),
  provenance: sampleProvenanceSchema,
});
export type CachedSampleInfo = z.infer<typeof cachedSampleInfoSchema>;
