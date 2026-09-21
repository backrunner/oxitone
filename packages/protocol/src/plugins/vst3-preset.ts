import { z } from "zod";
import { vst3ConfigurationSchema, vst3SourceSchema } from "./vst3.js";

/** Local reusable configuration. Trust policy is deliberately not imported from files. */
export const vst3PresetSchema = z
  .strictObject({
    formatVersion: z.literal(1),
    kind: z.literal("oxitone-vst3-preset"),
    name: z.string().min(1).max(256),
    source: vst3SourceSchema.omit({ allowPlugins: true, expectedHash: true }),
    configuration: vst3ConfigurationSchema,
  })
  .refine(
    (preset) => preset.source.classId.toLowerCase() === preset.configuration.classId.toLowerCase(),
    "preset class and configuration must match",
  );
export type Vst3Preset = z.infer<typeof vst3PresetSchema>;
