import { z } from "zod";
import { vst3InfoSchema, vst3RenderOptionsSchema, vst3SourceSchema } from "./vst3.js";
import { vst3TransportSchema } from "./vst3-transport.js";
import { vst3BusActivationSchema, vst3AudioPortsShape, validVst3AudioPorts } from "./vst3-buses.js";

/** Control-only initialization of the native bounded stream. PCM never crosses JavaScript. */
export const vst3StreamStartSchema = z.strictObject({
  streamProtocolVersion: z.literal(11),
  source: vst3SourceSchema,
  options: vst3RenderOptionsSchema
    .pick({
      sampleRate: true,
      blockSize: true,
      configuration: true,
      parameters: true,
      tempo: true,
      timeSignature: true,
    })
    .extend({
      processingMode: z.enum(["realtime", "offline"]).default("realtime"),
      /** Initial project position for a fresh processor; overrides tempo/signature during processing. */
      transport: vst3TransportSchema.optional(),
      busActivation: vst3BusActivationSchema.optional(),
      /** Capture bounded MIDI 1.0 channel output; unsupported output events fail the stream. */
      midiOutput: z.boolean().default(false),
    }),
});
export type Vst3StreamStart = z.input<typeof vst3StreamStartSchema>;

export const vst3StreamReadySchema = z
  .strictObject({
    classId: vst3InfoSchema.shape.classId,
    sha256: vst3InfoSchema.shape.sha256,
    category: vst3InfoSchema.shape.category,
    ...vst3AudioPortsShape,
    streamProtocolVersion: z.literal(11),
    sampleRate: z.number().int().min(8000).max(192000),
    blockSize: z.number().int().min(16).max(4096),
    latencyFrames: z.number().int().min(0).max(1920000),
    tailFrames: z.number().int().min(0).max(0xffffffff),
    helperTimeConstraint: z.boolean(),
    parameters: z
      .array(
        z.strictObject({
          id: z.number().int().min(0).max(0xffffffff),
          writable: z.boolean(),
          automatable: z.boolean(),
        }),
      )
      .max(4096),
  })
  .refine(validVst3AudioPorts, "inconsistent VST3 audio ports or unsupported outputless processor");
export type Vst3StreamReady = z.infer<typeof vst3StreamReadySchema>;

/** Native scheduler preparation only. Epoch-relative PCM and events stay entirely in Rust. */
export const vst3StreamScheduleSchema = z.strictObject({
  scheduleVersion: z.literal(1),
  epoch: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER),
  latencyBlocks: z.number().int().min(2).max(16),
});
export type Vst3StreamSchedule = z.infer<typeof vst3StreamScheduleSchema>;

/** Native lifetime manager configuration. All sessions share one audio format. */
export const vst3StreamManagerSchema = z.strictObject({
  managerVersion: z.literal(1),
  sampleRate: z.number().int().min(8000).max(192000),
  blockSize: z.number().int().min(16).max(4096),
  capacity: z.number().int().min(2).max(16),
});
export type Vst3StreamManager = z.infer<typeof vst3StreamManagerSchema>;
