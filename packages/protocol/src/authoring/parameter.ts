import { z } from "zod";

/** Plugin or built-in node parameter declaration (04-api-contracts.md). */
export const parameterSpecSchema = z.object({
  id: z.string().min(1),
  label: z.string(),
  unit: z.enum(["normalized", "db", "hz", "semitones", "seconds", "beats", "enum"]),
  min: z.number().finite(),
  max: z.number().finite(),
  default: z.number().finite(),
  smoothing: z.enum(["none", "linear", "one-pole"]),
  rate: z.enum(["control", "audio"]),
  automation: z.boolean().optional(),
  mapping: z.enum(["linear", "log", "bipolar", "enum"]).optional(),
});
export type ParameterSpec = z.infer<typeof parameterSpecSchema>;
