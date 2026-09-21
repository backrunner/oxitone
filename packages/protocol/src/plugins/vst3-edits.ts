import { z } from "zod";
import { vst3TransportSchema } from "./vst3-transport.js";

export const vst3EditCaptureIdSchema = z
  .string()
  .regex(/^[1-9][0-9]{0,19}$/)
  .refine((value) => /^[1-9][0-9]{0,19}$/.test(value) && BigInt(value) <= 0xffffffffffffffffn);
export const vst3EditSequenceSchema = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
export const vst3EditPositionSchema = z.strictObject({
  audioSequence: vst3EditSequenceSchema,
  transport: vst3TransportSchema.refine(
    (transport) => transport.playing,
    "Gesture position requires a playing audio block",
  ),
  reset: z.boolean(),
});
const edit = {
  sequence: vst3EditSequenceSchema,
  parameterId: z.number().int().min(0).max(0xffffffff),
  position: vst3EditPositionSchema,
};
export const vst3RecordingSelectionSchema = z.strictObject({
  mode: z.enum(["touch", "write"]),
  parameterIds: z
    .array(z.number().int().min(0).max(0xffffffff))
    .min(1)
    .max(32)
    .refine(
      (ids) => ids.every((id, index) => index === 0 || id > ids[index - 1]!),
      "Parameter IDs must be sorted and unique",
    ),
});
export const vst3RecordingSchema = vst3RecordingSelectionSchema.extend({
  sampleRate: z.number().int().min(8000).max(192000),
});
export const vst3ParameterEditSchema = z.discriminatedUnion("kind", [
  z.strictObject({ ...edit, kind: z.literal("begin") }),
  z.strictObject({ ...edit, kind: z.literal("value"), value: z.number().min(0).max(1) }),
  z.strictObject({ ...edit, kind: z.literal("end") }),
  z.strictObject({
    ...edit,
    kind: z.literal("sample"),
    value: z.number().min(0).max(1),
    frames: z.number().int().min(1).max(4096),
  }),
]);
export const vst3EditPageSchema = z
  .strictObject({
    captureId: vst3EditCaptureIdSchema,
    status: z.enum(["recording", "stopping", "stopped", "failed"]),
    firstSequence: vst3EditSequenceSchema,
    nextSequence: vst3EditSequenceSchema,
    pendingEvents: z.number().int().min(0).max(4096),
    events: z.array(vst3ParameterEditSchema).max(256),
    recording: vst3RecordingSchema.optional(),
    endPosition: vst3EditPositionSchema.optional(),
    error: z
      .strictObject({
        code: z.enum(["BudgetExceeded", "PluginConfigInvalid", "PluginRestartRequired"]),
        message: z.string().min(1),
      })
      .optional(),
  })
  .superRefine((page, ctx) => {
    if (
      page.nextSequence < page.firstSequence ||
      page.firstSequence + page.events.length > page.nextSequence ||
      page.events.some((event, index) => event.sequence !== page.firstSequence + index) ||
      (page.status === "failed") !== !!page.error ||
      (page.status === "stopped") !== !!page.endPosition ||
      (page.status === "failed" && page.events.length > 0) ||
      (!page.error && page.events.length !== Math.min(256, page.nextSequence - page.firstSequence)) ||
      page.events.some(
        (event, index) => index > 0 && event.position.audioSequence < page.events[index - 1]!.position.audioSequence,
      ) ||
      (page.endPosition &&
        page.events.some((event) => event.position.audioSequence > page.endPosition!.audioSequence)) ||
      page.events.some(
        (event) =>
          event.kind === "sample" &&
          (!page.recording?.parameterIds.includes(event.parameterId) ||
            (page.endPosition && event.position.audioSequence >= page.endPosition.audioSequence)),
      ) ||
      (["failed", "stopped"].includes(page.status) && page.pendingEvents !== 0)
    )
      ctx.addIssue({ code: "custom", message: "Invalid VST3 edit page sequence or lifecycle" });
  });
export type Vst3EditPosition = z.infer<typeof vst3EditPositionSchema>;
export type Vst3ParameterEdit = z.infer<typeof vst3ParameterEditSchema>;
export type Vst3EditPage = z.infer<typeof vst3EditPageSchema>;
export type Vst3Recording = z.infer<typeof vst3RecordingSchema>;
