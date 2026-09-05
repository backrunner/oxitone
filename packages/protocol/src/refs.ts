import { z } from "zod";
import { beatWireSchema } from "./beat.js";
import { entityIdSchema, frameWireSchema } from "./primitives.js";
import { fadeSpecSchema } from "./timeline.js";

export const trackSpecSchema = z.object({
  id: entityIdSchema,
  name: z.string().optional(),
  channelIds: z.array(entityIdSchema),
  tempo: z.number().finite().min(20).max(999).optional(),
  patternClipIds: z.array(entityIdSchema),
  sampleClipIds: z.array(entityIdSchema),
  enabled: z.boolean().optional(),
  midiChannel: z.number().int().min(1).max(16).optional(),
});
export type TrackSpec = z.infer<typeof trackSpecSchema>;

/** Non-destructive sample edit descriptor (baked at prepare, not automatable). */
export const sampleEditSpecSchema = z.object({
  startFrame: frameWireSchema.optional(),
  endFrame: frameWireSchema.optional(),
  level: z.number().finite().min(0).max(2).optional(),
  tone: z.number().finite().min(-1).max(1).optional(),
  normalize: z.object({ peakDb: z.number().finite() }).optional(),
  fadeIn: fadeSpecSchema.optional(),
  fadeOut: fadeSpecSchema.optional(),
  crossfade: fadeSpecSchema.optional(),
});
export type SampleEditSpec = z.infer<typeof sampleEditSpecSchema>;

export const sampleRefSchema = z.object({
  id: entityIdSchema,
  assetUri: z.string().min(1),
  sha256: z.string().regex(/^[0-9a-f]{64}$/, "sha256 must be lowercase hex"),
  format: z.enum(["wav", "aiff", "flac", "mp3", "mp4", "m4a"]),
  sampleRate: z.number().int().positive(),
  channels: z.union([z.literal(1), z.literal(2)]),
  frames: frameWireSchema,
  edits: sampleEditSpecSchema.optional(),
  musicalLengthBeats: beatWireSchema.optional(),
});
export type SampleRef = z.infer<typeof sampleRefSchema>;

export const instrumentRefSchema = z.object({
  pluginId: z.string().min(1),
  pluginVersion: z.string().min(1),
  parameters: z.record(z.string(), z.number().finite()),
  resources: z.record(z.string(), z.string()).optional(),
  state: z.unknown().optional(),
});
export type InstrumentRef = z.infer<typeof instrumentRefSchema>;

export const effectRefSchema = z.object({
  pluginId: z.string().min(1),
  pluginVersion: z.string().min(1),
  parameters: z.record(z.string(), z.number().finite()),
  resources: z.record(z.string(), z.string()).optional(),
  bypass: z.boolean().optional(),
  mix: z.number().finite().min(0).max(1).optional(),
});
export type EffectRef = z.infer<typeof effectRefSchema>;
