import { z } from "zod";
import { samplerOptionsSchema } from "./instruments.js";

const key = z.number().int().min(0).max(127);
const velocity = z.number().int().min(1).max(127);
export const multisamplerStateSchema = z.strictObject({
  version: z.literal(1),
  regions: z.array(z.strictObject({
    resource: z.string().min(1).max(128), rootKey: key,
    keyRange: z.tuple([key, key]), velocityRange: z.tuple([velocity, velocity]),
    gain: z.number().finite().min(0).max(4).default(1),
  })).min(1).max(256),
});
export const multisamplerOptionsSchema = samplerOptionsSchema.omit({ rootKey: true }).extend({
  transpose: z.number().finite().min(-48).max(48).optional(),
});
export type MultisamplerState = z.infer<typeof multisamplerStateSchema>;
export type MultisamplerOptions = z.infer<typeof multisamplerOptionsSchema>;
