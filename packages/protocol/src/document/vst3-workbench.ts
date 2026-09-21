import { z } from "zod";
import { vst3SourceSchema, vst3RenderOptionsSchema, vst3ParametersSchema } from "../plugins/vst3.js";
import { vst3RecordingStartSchema, vst3RecordingStopSchema, vst3RecordingCancelSchema } from "./vst3-recording.js";

/** Catalog discovery, offline tools and explicit source-backed audio import. */
export const vst3WorkbenchCommandSchema = z.discriminatedUnion("kind", [
  vst3RecordingStartSchema,
  vst3RecordingStopSchema,
  vst3RecordingCancelSchema,
  z.strictObject({ kind: z.literal("scan") }),
  z.strictObject({ kind: z.literal("add"), source: vst3SourceSchema.omit({ allowPlugins: true }) }),
  z.strictObject({ kind: z.literal("addBundle"), bundlePath: vst3SourceSchema.shape.bundlePath }),
  z.strictObject({ kind: z.literal("remove"), plugin: z.string().min(1).max(256) }),
  z.strictObject({ kind: z.literal("edit"), plugin: z.string().min(1).max(256), parameters: vst3ParametersSchema }),
  z.strictObject({
    kind: z.literal("controlInstance"),
    site: z.string().min(1).max(256),
    usage: z.string().min(1).max(256).optional(),
    action: z.enum(["openEditor", "closeEditor"]),
  }),
  z.strictObject({
    kind: z.literal("captureInstance"),
    site: z.string().min(1).max(256),
    usage: z.string().min(1).max(256).optional(),
  }),
  z.strictObject({
    kind: z.literal("render"),
    plugin: z.string().min(1).max(256),
    options: vst3RenderOptionsSchema.omit({ configuration: true }),
  }),
  z.strictObject({ kind: z.literal("cancel") }),
  z.strictObject({ kind: z.literal("loadPreset"), path: vst3SourceSchema.shape.bundlePath }),
  z.strictObject({
    kind: z.literal("savePreset"),
    plugin: z.string().min(1).max(256),
    path: vst3SourceSchema.shape.bundlePath,
    parameters: vst3ParametersSchema,
  }),
  z.strictObject({
    kind: z.literal("attachRender"),
    plugin: z.string().min(1).max(256),
    name: z.string().min(1).max(256),
    startBeat: z.number().finite().min(0).max(1_000_000_000),
  }),
]);
export type Vst3WorkbenchCommand = z.infer<typeof vst3WorkbenchCommandSchema>;
