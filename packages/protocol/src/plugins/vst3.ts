import { z } from "zod";
import { vst3AudioPortsShape, validVst3AudioPorts } from "./vst3-buses.js";

/** Independent helper protocol; VST3 code remains in isolated native processes. */
export const VST3_PROTOCOL_VERSION = 1 as const;
export const VST3_MAX_STATE_BYTES = 4 * 1024 * 1024;
const path = z
  .string()
  .min(1)
  .max(4096)
  .refine((value) => !value.includes("\0"));
const hash = z.string().regex(/^[a-f0-9]{64}$/);
export const vst3ClassIdSchema = z.string().regex(/^[a-fA-F0-9]{32}$/);
export const vst3SourceSchema = z.strictObject({
  bundlePath: path,
  /** Exact audio class, in Steinberg's 32-hex-character representation; no UID replacement. */
  classId: vst3ClassIdSchema,
  expectedHash: hash.optional(),
  allowPlugins: z.enum(["signed-only", "any"]).default("signed-only"),
});
export type Vst3Source = z.input<typeof vst3SourceSchema>;

const normalized = z.number().finite().min(0).max(1);
const parameterId = z
  .string()
  .regex(/^(0|[1-9][0-9]{0,9})$/)
  .refine((id) => Number(id) <= 0xffff_ffff);
export const vst3ParametersSchema = z
  .record(parameterId, normalized)
  .refine((value) => Object.keys(value).length <= 4096);
export const vst3ConfigurationSchema = z.strictObject({
  formatVersion: z.literal(1),
  classId: vst3ClassIdSchema,
  sha256: hash,
  /** Opaque component + controller state captured on the helper's main thread between process calls. */
  stateBase64: z
    .string()
    .max(4 * Math.ceil(VST3_MAX_STATE_BYTES / 3))
    .regex(/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/),
  parameters: vst3ParametersSchema,
});
export type Vst3Configuration = z.infer<typeof vst3ConfigurationSchema>;

export const vst3EventSchema = z.discriminatedUnion("type", [
  z.strictObject({
    type: z.literal("sysEx"),
    frame: z.number().int().nonnegative().max(536_000_000),
    data: z
      .array(z.number().int().min(0).max(255))
      .min(2)
      .max(4096)
      .refine(
        (bytes) => bytes[0] === 0xf0 && bytes.at(-1) === 0xf7 && bytes.slice(1, -1).every((b) => b < 128),
        "SysEx requires a complete F0...F7 message with 7-bit data",
      ),
  }),
  z.strictObject({
    type: z.literal("midi"),
    frame: z.number().int().nonnegative().max(536_000_000),
    message: z
      .tuple([z.number().int().min(0x80).max(0xef), z.number().int().min(0).max(127), z.number().int().min(0).max(127)])
      .refine(([status, , value]) => ![0xc0, 0xd0].includes(status & 0xf0) || value === 0),
  }),
  z.strictObject({
    type: z.literal("parameter"),
    frame: z.number().int().nonnegative().max(536_000_000),
    parameterId: z.number().int().min(0).max(0xffff_ffff),
    value: normalized,
  }),
  z.strictObject({
    type: z.enum(["noteOn", "noteOff"]),
    frame: z.number().int().nonnegative().max(536_000_000),
    channel: z.number().int().min(0).max(15),
    pitch: z.number().int().min(0).max(127),
    velocity: normalized,
  }),
]);
export type Vst3Event = z.infer<typeof vst3EventSchema>;

export const vst3RenderOptionsSchema = z.strictObject({
  path,
  sampleRate: z.number().int().min(8000).max(192000).default(48000),
  blockSize: z.number().int().min(16).max(4096).default(128),
  /** Content frames. Tail is additional silence-fed processing; events must precede this frame. */
  frames: z.number().int().positive().max(536_000_000),
  tailFrames: z
    .number()
    .int()
    .nonnegative()
    .max(192000 * 60)
    .default(0),
  inputPath: path.optional(),
  configuration: vst3ConfigurationSchema.optional(),
  parameters: vst3ParametersSchema.default({}),
  events: z.array(vst3EventSchema).max(100_000).default([]),
  tempo: z.number().finite().min(20).max(999).default(120),
  timeSignature: z
    .tuple([
      z.number().int().min(1).max(32),
      z.union([z.literal(1), z.literal(2), z.literal(4), z.literal(8), z.literal(16)]),
    ])
    .default([4, 4]),
});
export type Vst3RenderOptions = z.input<typeof vst3RenderOptionsSchema>;

export const vst3InfoSchema = z
  .strictObject({
    protocolVersion: z.literal(VST3_PROTOCOL_VERSION),
    classId: vst3ClassIdSchema,
    name: z.string(),
    vendor: z.string(),
    version: z.string(),
    category: z.string(),
    sha256: hash,
    ...vst3AudioPortsShape,
    parameters: z
      .array(
        z.strictObject({
          id: z.number().int().min(0).max(0xffff_ffff),
          name: z.string(),
          unit: z.string(),
          value: normalized,
          default: normalized,
          /** VST3 gaps: 0 continuous; 1 two states; n means n+1 states. */
          stepCount: z.number().int().nonnegative(),
          canAutomate: z.boolean(),
          readOnly: z.boolean(),
        }),
      )
      .max(4096),
    configuration: vst3ConfigurationSchema.nullable(),
  })
  .refine(validVst3AudioPorts, "inconsistent VST3 audio ports or unsupported outputless processor");
export type Vst3Info = z.infer<typeof vst3InfoSchema>;
export const vst3RenderReportSchema = z.strictObject({
  protocolVersion: z.literal(VST3_PROTOCOL_VERSION),
  path,
  frames: z.number().int().positive(),
  sampleRate: z.number().int().positive(),
  sha256: hash,
  pluginSha256: hash,
  latencyFrames: z.number().int().nonnegative(),
  tailFrames: z.number().int().nonnegative(),
  peak: z.number().finite().nonnegative(),
});
export type Vst3RenderReport = z.infer<typeof vst3RenderReportSchema>;
