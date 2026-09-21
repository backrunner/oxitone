import { z } from "zod";
import { parameterSpecSchema } from "../authoring/parameter.js";
import { vst3SourceSchema, vst3InfoSchema } from "./vst3.js";

/** Register an isolated VST3 factory for project instruments and inserts. */
export const registerVst3OptionsSchema = z.strictObject({
  registrationVersion: z.literal(1),
  source: vst3SourceSchema,
  /** Inspection metadata; native registration rechecks class, hash, layout, IDs and flags. */
  metadata: vst3InfoSchema,
  helperPath: z
    .string()
    .min(1)
    .max(4096)
    .refine((path) => !path.includes("\0")),
});
export type RegisterVst3Options = z.input<typeof registerVst3OptionsSchema>;

/** Identity is derived from the exact class and binary, never an invented C ABI manifest. */
export const registeredVst3Schema = z.strictObject({
  registrationVersion: z.literal(1),
  pluginId: z.string().regex(/^vst3\.[a-f0-9]{32}$/),
  pluginVersion: z.string().regex(/^0\.0\.0\+[a-f0-9]{64}$/),
  sha256: z.string().regex(/^[a-f0-9]{64}$/),
  kind: z.enum(["instrument", "effect"]),
  parameters: z.array(parameterSpecSchema).max(4096),
});
export type RegisteredVst3 = z.infer<typeof registeredVst3Schema>;
