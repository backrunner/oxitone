import { z } from "zod";
import { vst3ClassIdSchema, vst3SourceSchema } from "./vst3.js";

export const vst3BundleSourceSchema = vst3SourceSchema.omit({ classId: true });
export type Vst3BundleSource = z.input<typeof vst3BundleSourceSchema>;
export const vst3ClassListSchema = z.strictObject({
  protocolVersion: z.literal(1),
  bundlePath: vst3BundleSourceSchema.shape.bundlePath,
  sha256: z.string().regex(/^[a-f0-9]{64}$/),
  vendor: z.string(),
  classes: z
    .array(
      z.strictObject({
        classId: vst3ClassIdSchema,
        name: z.string(),
        category: z.literal("Audio Module Class"),
        version: z.string(),
      }),
    )
    .max(1024)
    .refine((classes) => new Set(classes.map((c) => c.classId.toLowerCase())).size === classes.length),
});
export type Vst3ClassList = z.infer<typeof vst3ClassListSchema>;
