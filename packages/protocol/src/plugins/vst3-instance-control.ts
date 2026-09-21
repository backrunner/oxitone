import { z } from "zod";
import { entityIdSchema } from "../base/primitives.js";
import { vst3ControlCommandSchema, vst3ControlStateSchema } from "./vst3-control.js";

const generationPattern = /^[1-9][0-9]{0,19}$/;
const graphGeneration = z
  .string()
  .regex(generationPattern)
  .refine((v) => generationPattern.test(v) && BigInt(v) <= 0xffffffffffffffffn);
export const vst3InstanceTargetSchema = z.strictObject({ graphGeneration, instanceId: entityIdSchema });
export const vst3InstanceInventorySchema = z.strictObject({
  instanceControlVersion: z.literal(1),
  graphGeneration,
  state: z.enum(["prepared", "active", "retired"]),
  instances: z.array(
    z.strictObject({ instanceId: entityIdSchema, pluginId: z.string().min(1), pluginVersion: z.string().min(1) }),
  ),
});
export const vst3InstanceRequestSchema = vst3InstanceTargetSchema.extend({
  instanceControlVersion: z.literal(1),
  command: vst3ControlCommandSchema,
  timeoutMs: z.number().int().min(1).max(600_000).default(5000),
});
export const vst3InstanceResultSchema = vst3InstanceTargetSchema.extend({
  instanceControlVersion: z.literal(1),
  state: vst3ControlStateSchema,
});
export type Vst3InstanceTarget = z.infer<typeof vst3InstanceTargetSchema>;
export type Vst3InstanceInventory = z.infer<typeof vst3InstanceInventorySchema>;
export type Vst3InstanceRequest = z.input<typeof vst3InstanceRequestSchema>;
export type Vst3InstanceResult = z.infer<typeof vst3InstanceResultSchema>;
