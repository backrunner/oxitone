import { z } from "zod";
import { beatWireSchema } from "./beat.js";
import { entityIdSchema, frameWireSchema, timecodeSchema } from "./primitives.js";

export const engineOptionsSchema = z.object({
  sampleRate: z.number().int().positive().optional(),
  blockSize: z.union([z.literal(64), z.literal(128), z.literal(256)]).optional(),
  renderAheadBlocks: z.number().int().min(2).max(16).optional(),
  latencyMode: z.enum(["buffered", "direct"]).optional(),
  allowPlugins: z.enum(["signed-only", "any"]).optional(),
  outputDeviceId: z.string().optional(),
  audioBackend: z.enum(["device", "simulated"]).optional(),
  deviceRatePolicy: z.enum(["adapt-device", "resample"]).optional(),
  deviceChangePolicy: z.enum(["follow-default", "pause"]).optional(),
  metronome: z
    .object({ enabled: z.boolean(), level: z.number().finite().min(0).max(1).optional() })
    .optional(),
});
export type EngineOptions = z.infer<typeof engineOptionsSchema>;

/** Control-thread asset resolution for native graph compilation. */
export const compileOptionsSchema = z.object({ assetBaseDir: z.string().min(1).optional() });
export type CompileOptions = z.infer<typeof compileOptionsSchema>;

/** Render range boundary: exactly one of bar / beat / timecode / marker. */
export const renderPositionSchema = z.union([
  z.object({ bar: z.number().int().min(1) }),
  z.object({ beat: beatWireSchema }),
  timecodeSchema,
  z.object({ marker: entityIdSchema }),
]);
export type RenderPosition = z.infer<typeof renderPositionSchema>;

export const renderOptionsSchema = z.object({
  path: z.string().min(1),
  start: renderPositionSchema.optional(),
  end: renderPositionSchema.optional(),
  sampleRate: z.number().int().positive().optional(),
  blockSize: z.union([z.literal(64), z.literal(128), z.literal(256)]).optional(),
  tailSeconds: z.number().finite().nonnegative().optional(),
  respectSolo: z.boolean().optional(),
  seed: z.number().int().nonnegative().optional(),
  bitDepth: z.union([z.literal(16), z.literal(24), z.literal("float32")]).optional(),
  dither: z.enum(["tpdf", "none"]).optional(),
  stems: z.enum(["none", "mixer-channels", "tracks"]).optional(),
  includeMetronome: z.boolean().optional(),
  /** Base directory for resolving relative `SampleRef.assetUri` values during the render. */
  assetBaseDir: z.string().min(1).optional(),
});
export type RenderOptions = z.infer<typeof renderOptionsSchema>;

export const renderFileReportSchema = z.object({
  path: z.string(),
  stem: entityIdSchema.optional(),
  durationSeconds: z.number().finite().nonnegative(),
  peakDbfs: z.number().finite(),
  truePeakDbfs: z.number().finite(),
  integratedLufs: z.number().finite(),
});

export const renderReportSchema = z.object({
  files: z.array(renderFileReportSchema),
  graphLatencyFrames: frameWireSchema,
});
export type RenderReport = z.infer<typeof renderReportSchema>;

/** Audio output device descriptor (04-api-contracts.md). Empty until M4 device I/O. */
export const outputDeviceInfoSchema = z.object({
  id: z.string().min(1),
  name: z.string(),
  nominalSampleRates: z.array(z.number().int().positive()),
  bufferFrameSizeRange: z.tuple([z.number().int().positive(), z.number().int().positive()]),
  isDefault: z.boolean(),
});
export type OutputDeviceInfo = z.infer<typeof outputDeviceInfoSchema>;

/** Ring horizon + device latency in a frames/seconds dual representation. */
export const outputLatencySchema = z.object({
  frames: frameWireSchema,
  seconds: z.number().finite().nonnegative(),
  breakdown: z.object({
    ring: frameWireSchema,
    resampler: frameWireSchema,
    deviceBuffer: frameWireSchema,
    safetyOffset: frameWireSchema,
    deviceLatency: frameWireSchema,
  }),
});
export type OutputLatency = z.infer<typeof outputLatencySchema>;

/** One structured diagnostic event from the realtime engine. */
export const engineDiagnosticEventSchema = z.object({
  code: z.string().min(1),
  severity: z.enum(["warning", "error"]),
  frame: frameWireSchema,
  message: z.string(),
});
export type EngineDiagnosticEvent = z.infer<typeof engineDiagnosticEventSchema>;

/**
 * Realtime counters and drained events (05-performance-and-benchmarks.md
 * §诊断). Event delivery is destructive: each call returns the events
 * raised since the previous call.
 */
export const engineDiagnosticsSchema = z.object({
  state: z.enum(["stopped", "playing", "paused", "rendering"]),
  cursor: frameWireSchema,
  blocks: z.number().int().nonnegative(),
  deadlineMisses: z.number().int().nonnegative(),
  xruns: z.number().int().nonnegative(),
  nanBlocks: z.number().int().nonnegative(),
  queueDrops: z.number().int().nonnegative(),
  performanceWarnings: z.number().int().nonnegative(),
  engineLoad: z.number().finite().nonnegative(),
  ringOccupancyFrames: z.number().int().nonnegative(),
  blockTimeNs: z.object({
    p50: z.number().int().nonnegative(),
    p95: z.number().int().nonnegative(),
    p99: z.number().int().nonnegative(),
    max: z.number().int().nonnegative(),
  }),
  events: z.array(engineDiagnosticEventSchema),
});
export type EngineDiagnostics = z.infer<typeof engineDiagnosticsSchema>;

export const MIDI_DEFAULT_PPQ = 960;
export const MIDI_MAX_PPQ = 0x7fff;

export const midiExportOptionsSchema = z.object({
  path: z.string().min(1).optional(),
  ppq: z.number().int().min(1).max(MIDI_MAX_PPQ).optional(),
  tempoEventResolutionTicks: z.number().int().min(1).optional(),
});
export type MidiExportOptions = z.infer<typeof midiExportOptionsSchema>;

export const midiChannelAssignmentSchema = z.object({
  trackId: entityIdSchema,
  channel: z.number().int().min(1).max(16),
  source: z.enum(["explicit", "auto"]),
});
export type MidiChannelAssignment = z.infer<typeof midiChannelAssignmentSchema>;

export const midiSkippedAutomationSchema = z.object({
  laneId: entityIdSchema,
  targetEntityId: entityIdSchema,
  targetParameterId: z.string(),
  reason: z.string(),
});
export type MidiSkippedAutomation = z.infer<typeof midiSkippedAutomationSchema>;

export const midiDiagnosticsSchema = z.object({
  ppq: z.number().int().min(1).max(MIDI_MAX_PPQ),
  tempoEventResolutionTicks: z.number().int().min(1),
  tempoEventCount: z.number().int().nonnegative(),
  noteTrackCount: z.number().int().nonnegative(),
  channelAssignments: z.array(midiChannelAssignmentSchema),
  skippedAutomation: z.array(midiSkippedAutomationSchema),
});
export type MidiDiagnostics = z.infer<typeof midiDiagnosticsSchema>;

/** `path`/`bytes` are present when the export wrote a file, `bytesBase64` otherwise. */
export const midiExportReportSchema = z.object({
  path: z.string().optional(),
  bytes: z.number().int().nonnegative().optional(),
  bytesBase64: z.string().optional(),
  diagnostics: midiDiagnosticsSchema,
});
export type MidiExportReport = z.infer<typeof midiExportReportSchema>;
