import { z } from "zod";
import { instrumentRefSchema, effectRefSchema, sampleRefSchema } from "./refs.js";

const common = {
  formatVersion: z.literal("1.0"), abiMajor: z.literal(1), name: z.string().optional(),
  samples: z.array(sampleRefSchema).default([]),
};
export const presetSchema = z.discriminatedUnion("kind", [
  instrumentRefSchema.omit({ instanceId: true }).extend({ ...common, kind: z.literal("instrument") }),
  effectRefSchema.omit({ instanceId: true }).extend({ ...common, kind: z.literal("effect") }),
  z.object({ ...common, kind: z.literal("channel"), instrument: instrumentRefSchema.omit({ instanceId: true }),
    effectChain: z.array(effectRefSchema.omit({ instanceId: true })), level: z.number().finite().min(0).max(2),
    pan: z.number().finite().min(-1).max(1), swing: z.number().finite().min(0).max(1),
  }),
]);
export type Preset = z.infer<typeof presetSchema>;
export type ChannelPreset = Extract<Preset, { kind: "channel" }>;
export type InstrumentPreset = Extract<Preset, { kind: "instrument" }>;
export type EffectPreset = Extract<Preset, { kind: "effect" }>;
