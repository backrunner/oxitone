import { z } from "zod";

export const sampleFormatSchema = z.enum(["wav", "aiff", "flac", "mp3", "mp4", "m4a"]);

/** Informational source history; only SampleRef.sha256 identifies playable bytes. */
export const sampleProvenanceSchema = z.object({
  sourceSha256: z.string().regex(/^[0-9a-f]{64}$/),
  sourceFormat: sampleFormatSchema,
  sourceSampleRate: z.number().int().positive().max(0xffff_ffff),
  sourceChannels: z.number().int().min(1).max(255),
  sourceBitDepth: z.number().int().min(1).max(255).optional(),
  decoder: z.string().min(1),
  channelLayoutAction: z.enum(["kept", "downmixed-to-stereo"]),
  cacheEncoding: z.literal("wav-f32-v1").optional(),
});
export type SampleProvenance = z.infer<typeof sampleProvenanceSchema>;
