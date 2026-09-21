import { z } from "zod";
import { vst3RecordingSelectionSchema } from "../plugins/vst3-edits.js";
const handle = z.string().min(1).max(256);
export const vst3RecordingStartSchema = vst3RecordingSelectionSchema.extend({
  kind: z.literal("startRecording"),
  site: handle,
  usage: handle.optional(),
});
export const vst3RecordingStopSchema = z.strictObject({ kind: z.literal("stopRecording"), recordingId: handle });
export const vst3RecordingCancelSchema = z.strictObject({ kind: z.literal("cancelRecording"), recordingId: handle });
/** Session state only; never contains a take, raw native pages or opaque plugin state. */
export const vst3DocumentRecordingSchema = vst3RecordingSelectionSchema.extend({
  id: handle,
  instanceId: handle,
  status: z.enum(["starting", "recording", "stopping", "captured", "failed"]),
  error: z.strictObject({ code: z.string().min(1), message: z.string().min(1) }).optional(),
});
export type Vst3DocumentRecording = z.infer<typeof vst3DocumentRecordingSchema>;
