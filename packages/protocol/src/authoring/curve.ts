import { z } from "zod";
import { beatWireSchema } from "../base/beat.js";

/** Interpolation between two automation points (04-api-contracts.md). */
export const curveSchema = z.union([
  z.object({ kind: z.enum(["step", "linear", "smooth", "exponential"]) }),
  z.object({
    kind: z.literal("bezier"),
    out: z.tuple([z.number().finite(), z.number().finite()]),
    in: z.tuple([z.number().finite(), z.number().finite()]),
  }),
]);
export type Curve = z.infer<typeof curveSchema>;

export const CURVE_KINDS = ["step", "linear", "smooth", "exponential", "bezier"] as const;
export type CurveKind = (typeof CURVE_KINDS)[number];

export const automationPointSchema = z.object({
  beat: beatWireSchema,
  value: z.number().finite(),
  curve: curveSchema.optional(),
});
export type AutomationPoint = z.infer<typeof automationPointSchema>;
