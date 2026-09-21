import { z } from "zod";
import { vst3InfoSchema } from "./vst3.js";
import {
  vst3EditCaptureIdSchema,
  vst3EditPageSchema,
  vst3EditSequenceSchema,
  vst3RecordingSelectionSchema,
} from "./vst3-edits.js";

/** Native stream control only. Audio and sample-accurate automation stay in Rust. */
export const vst3ControlCommandSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("startEdits") }),
  vst3RecordingSelectionSchema.extend({ kind: z.literal("startRecording") }),
  z.strictObject({
    kind: z.literal("readEdits"),
    captureId: vst3EditCaptureIdSchema,
    fromSequence: vst3EditSequenceSchema,
  }),
  z.strictObject({
    kind: z.literal("stopEdits"),
    captureId: vst3EditCaptureIdSchema,
    fromSequence: vst3EditSequenceSchema,
  }),
  z.strictObject({ kind: z.literal("discardEdits"), captureId: vst3EditCaptureIdSchema }),
  z.strictObject({ kind: z.literal("openEditor") }),
  z.strictObject({ kind: z.literal("closeEditor") }),
  z.strictObject({ kind: z.literal("poll") }),
  z.strictObject({ kind: z.literal("capture") }),
  z.strictObject({
    kind: z.literal("setParameter"),
    parameterId: z.number().int().min(0).max(0xffffffff),
    value: z.number().min(0).max(1),
  }),
]);
export const vst3ControlRequestSchema = z.strictObject({
  controlProtocolVersion: z.literal(1),
  command: vst3ControlCommandSchema,
});
/** The old stream is frozen. Capture its state before compiling a replacement graph. */
export const vst3RestartSchema = z.strictObject({
  reasons: z
    .array(z.enum(["io", "latency", "parameters", "reload"]))
    .min(1)
    .max(4)
    .refine((reasons) => new Set(reasons).size === reasons.length),
  latencyFrames: z.number().int().min(0).max(0xffffffff),
  tailFrames: z.number().int().min(0).max(0xffffffff),
});
export const vst3ControlStateSchema = z.strictObject({
  editorOpen: z.boolean(),
  /** The next audio packet sequence; not the device's audible playhead. */
  nextSequence: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER),
  info: vst3InfoSchema.optional(),
  edits: vst3EditPageSchema.optional(),
  restart: vst3RestartSchema.optional(),
});
export const vst3ControlResponseSchema = z.discriminatedUnion("ok", [
  z.strictObject({ controlProtocolVersion: z.literal(1), ok: z.literal(true), state: vst3ControlStateSchema }),
  z.strictObject({
    controlProtocolVersion: z.literal(1),
    ok: z.literal(false),
    error: z.strictObject({ code: z.string().min(1), message: z.string() }),
  }),
]);
export type Vst3ControlCommand = z.infer<typeof vst3ControlCommandSchema>;
export type Vst3ControlRequest = z.infer<typeof vst3ControlRequestSchema>;
export type Vst3ControlState = z.infer<typeof vst3ControlStateSchema>;
export type Vst3ControlResponse = z.infer<typeof vst3ControlResponseSchema>;
export type Vst3Restart = z.infer<typeof vst3RestartSchema>;
