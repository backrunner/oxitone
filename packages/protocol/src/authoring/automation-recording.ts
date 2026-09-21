import { z } from "zod";

const index = z.number().int().min(0);
const identity = { pluginId: z.string().startsWith("vst3."), pluginVersion: z.string().min(1) };
/** Normal source coordinates, never execution/capture IDs. */
export const automationRecordingTargetSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("instrument"), index, ...identity }),
  z.strictObject({ kind: z.literal("channelInsert"), index, slot: index, ...identity }),
  z.strictObject({ kind: z.literal("busInsert"), index, slot: index, ...identity }),
]);
export const recordedParameterSpanSchema = z
  .strictObject({
    parameterId: z.number().int().min(0).max(0xffffffff),
    start: z.number().finite().min(0),
    end: z.number().finite().positive(),
    value: z.number().finite().min(0).max(1),
  })
  .refine((span) => span.end > span.start, "recorded interval must have positive duration");
export const automationRecordingEditSchema = z
  .strictObject({
    target: automationRecordingTargetSchema,
    parameters: z
      .array(
        z.strictObject({
          id: z.number().int().min(0).max(0xffffffff),
          name: z.string().min(1).max(256),
        }),
      )
      .min(1)
      .max(32),
    spans: z.array(recordedParameterSpanSchema).max(32768),
  })
  .superRefine((edit, ctx) => {
    const ids = edit.parameters.map((parameter) => parameter.id);
    if (new Set(ids).size !== ids.length || edit.spans.some((span) => !ids.includes(span.parameterId)))
      ctx.addIssue({ code: "custom", message: "recording parameters must be unique and cover every span" });
  });
export type AutomationRecordingEdit = z.infer<typeof automationRecordingEditSchema>;
export type AutomationRecordingTarget = z.infer<typeof automationRecordingTargetSchema>;
export type RecordedParameterSpan = z.infer<typeof recordedParameterSpanSchema>;
