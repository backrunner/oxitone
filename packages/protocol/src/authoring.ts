import { z } from "zod";
import { beatWireSchema } from "./beat.js";
import { automationSourceSchema } from "./automation-source.js";
import { entityIdSchema, pitchSchema } from "./primitives.js";
import { effectRefSchema, instrumentRefSchema } from "./refs.js";
import { loopSpecSchema } from "./timeline.js";

export const noteSpecSchema = z.object({
  id: entityIdSchema.optional(),
  pitch: pitchSchema,
  start: beatWireSchema,
  duration: beatWireSchema,
  velocity: z.number().finite().min(0).max(1),
  offVelocity: z.number().finite().min(0).max(1).optional(),
  chance: z.number().finite().min(0).max(1).optional(),
  voice: z.number().int().nonnegative().optional(),
  tags: z.array(z.string()).optional(),
});
export type NoteSpec = z.infer<typeof noteSpecSchema>;

export const patternSpecSchema = z.object({
  id: entityIdSchema,
  name: z.string().optional(),
  lengthBeats: beatWireSchema,
  notes: z.array(noteSpecSchema),
  /** Independent Channel parts; leaves share the root loop period and may be shorter. */
  parts: z.array(z.object({ channelId: entityIdSchema, patternId: entityIdSchema })).min(1).max(256).optional(),
});
export type PatternSpec = z.infer<typeof patternSpecSchema>;

export const patternClipSpecSchema = z
  .object({
    id: entityIdSchema,
    patternId: entityIdSchema,
    trackId: entityIdSchema,
    startBeat: beatWireSchema,
    durationBeats: beatWireSchema.optional(),
    loopCount: z.number().int().min(1).optional(),
    lastBeat: beatWireSchema.optional(),
    transpose: z.number().int().optional(),
    velocityScale: z.number().finite().min(0).max(2).optional(),
    probability: z.number().finite().min(0).max(1).optional(),
    enabled: z.boolean().optional(),
  })
  .refine((c) => !(c.loopCount !== undefined && c.lastBeat !== undefined), {
    message: "loopCount and lastBeat are mutually exclusive",
  });
export type PatternClipSpec = z.infer<typeof patternClipSpecSchema>;

export const sampleClipSpecSchema = z.object({
  id: entityIdSchema,
  sampleId: entityIdSchema,
  trackId: entityIdSchema,
  startBeat: beatWireSchema,
  durationBeats: beatWireSchema.optional(),
  gain: z.number().finite().min(0).max(2).optional(),
  pan: z.number().finite().min(-1).max(1).optional(),
  rate: z.number().finite().min(0.25).max(4).optional(),
  loop: loopSpecSchema.optional(),
  tempoSync: z.enum(["off", "stretch", "repitch"]).optional(),
  stretchAlgorithm: z.string().min(1).optional(),
  enabled: z.boolean().optional(),
});
export type SampleClipSpec = z.infer<typeof sampleClipSpecSchema>;

export const channelSpecSchema = z.object({
  id: entityIdSchema,
  name: z.string().optional(),
  instrument: instrumentRefSchema,
  effectChain: z.array(effectRefSchema),
  level: z.number().finite().min(0).max(2),
  pan: z.number().finite().min(-1).max(1),
  swing: z.number().finite().min(0).max(1).optional(),
  mixerChannelId: entityIdSchema,
  mute: z.boolean().optional(),
  solo: z.boolean().optional(),
});
export type ChannelSpec = z.infer<typeof channelSpecSchema>;

export const sendSpecSchema = z.object({
  destinationId: entityIdSchema,
  ratio: z.number().finite().min(0).max(1),
  preFader: z.boolean().optional(),
  sidechain: z.boolean().optional(),
});
export type SendSpec = z.infer<typeof sendSpecSchema>;

export const mixerChannelSpecSchema = z.object({
  id: entityIdSchema,
  name: z.string().optional(),
  level: z.number().finite().min(0).max(2),
  balance: z.number().finite().min(-1).max(1),
  masterSendRatio: z.number().finite().min(0).max(1).optional(),
  inserts: z.array(effectRefSchema),
  sends: z.array(sendSpecSchema),
  mute: z.boolean().optional(),
  solo: z.boolean().optional(),
});
export type MixerChannelSpec = z.infer<typeof mixerChannelSpecSchema>;

export const automationLaneSpecSchema = z.object({
  id: entityIdSchema,
  target: z.object({ entityId: entityIdSchema, parameterId: z.string().min(1), scope: z.enum(["plugin", "effectHost"]).optional() }),
  source: automationSourceSchema,
  combine: z.enum(["replace", "add", "multiply", "max"]).optional(),
  playback: z.enum(["global", "playlist"]).optional(),
  loop: loopSpecSchema.optional(),
  lastBeat: beatWireSchema.optional(),
});
export type AutomationLaneSpec = z.infer<typeof automationLaneSpecSchema>;

export const automationClipSpecSchema = z.object({
  id: entityIdSchema,
  laneId: entityIdSchema,
  trackId: entityIdSchema,
  startBeat: beatWireSchema,
  durationBeats: beatWireSchema.optional(),
  enabled: z.boolean().optional(),
});
export type AutomationClipSpec = z.infer<typeof automationClipSpecSchema>;
