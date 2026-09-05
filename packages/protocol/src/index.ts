export { ErrorCode, ERROR_CODES, OxitoneError } from "./errors.js";
export type { OxitoneErrorCode, OxitoneErrorDetails } from "./errors.js";
export {
  PROTOCOL_VERSION,
  PROTOCOL_MAJOR,
  PROTOCOL_MINOR,
  checkProtocolVersion,
} from "./version.js";
export {
  beatWireSchema,
  beatToWire,
  beatFromWire,
  rationalFromF64,
  BEAT_MAX_DENOMINATOR,
  BEAT_MAX_NUMERATOR,
} from "./beat.js";
export type { Beat, BeatWire } from "./beat.js";
export {
  entityIdSchema,
  pitchSchema,
  frameWireSchema,
  frameToWire,
  frameFromWire,
  timecodeSchema,
  ID_PREFIXES,
} from "./primitives.js";
export type { EntityId, Pitch, FrameWire, Timecode } from "./primitives.js";
export { curveSchema, automationPointSchema, CURVE_KINDS } from "./curve.js";
export type { Curve, CurveKind, AutomationPoint } from "./curve.js";
export { parameterSpecSchema } from "./parameter.js";
export type { ParameterSpec } from "./parameter.js";
export {
  automationSourceSchema,
  chanceSourceSchema,
  gateSourceSchema,
  waveSourceSchema,
  curveSourceSchema,
  constantSourceSchema,
  chanceOptionsSchema,
  chanceOptionsToWire,
  WAVE_KINDS,
} from "./automation-source.js";
export type { AutomationSourceSpec, ChanceOptions, WaveKind } from "./automation-source.js";
export {
  tempoSegmentSchema,
  timeSignatureSegmentSchema,
  loopSpecSchema,
  markerSpecSchema,
  fadeSpecSchema,
} from "./timeline.js";
export type {
  TempoSegment,
  TimeSignatureSegment,
  LoopSpec,
  MarkerSpec,
  FadeSpec,
} from "./timeline.js";
export {
  trackSpecSchema,
  sampleEditSpecSchema,
  sampleRefSchema,
  instrumentRefSchema,
  effectRefSchema,
} from "./refs.js";
export type {
  TrackSpec,
  SampleEditSpec,
  SampleRef,
  InstrumentRef,
  EffectRef,
} from "./refs.js";
export {
  noteSpecSchema,
  patternSpecSchema,
  patternClipSpecSchema,
  sampleClipSpecSchema,
  channelSpecSchema,
  sendSpecSchema,
  mixerChannelSpecSchema,
  automationLaneSpecSchema,
} from "./authoring.js";
export type {
  NoteSpec,
  PatternSpec,
  PatternClipSpec,
  SampleClipSpec,
  ChannelSpec,
  SendSpec,
  MixerChannelSpec,
  AutomationLaneSpec,
} from "./authoring.js";
export {
  projectSnapshotSchema,
  decodeProjectSnapshot,
  encodeProjectSnapshot,
} from "./snapshot.js";
export type { ProjectSnapshot } from "./snapshot.js";
export {
  nativeCommandSchema,
  nativeEventSchema,
  transportCommandSchema,
  transportStateSchema,
  NATIVE_EVENT_TYPES,
  TRANSPORT_STATES,
} from "./commands.js";
export type { NativeCommand, NativeEvent, TransportCommand, TransportState } from "./commands.js";
export {
  engineOptionsSchema,
  renderPositionSchema,
  renderOptionsSchema,
  renderFileReportSchema,
  renderReportSchema,
  outputDeviceInfoSchema,
  outputLatencySchema,
  engineDiagnosticEventSchema,
  engineDiagnosticsSchema,
  midiExportOptionsSchema,
  midiChannelAssignmentSchema,
  midiSkippedAutomationSchema,
  midiDiagnosticsSchema,
  midiExportReportSchema,
  MIDI_DEFAULT_PPQ,
  MIDI_MAX_PPQ,
} from "./options.js";
export type {
  EngineOptions,
  RenderPosition,
  RenderOptions,
  RenderReport,
  OutputDeviceInfo,
  OutputLatency,
  EngineDiagnosticEvent,
  EngineDiagnostics,
  MidiExportOptions,
  MidiChannelAssignment,
  MidiSkippedAutomation,
  MidiDiagnostics,
  MidiExportReport,
} from "./options.js";
export { canonicalize, canonicalEncode } from "./canonical.js";
export {
  Pcg32,
  PCG32_MULTIPLIER,
  PCG32_INCREMENT,
  hash64,
  hash64Input,
} from "./pcg32.js";
export type { Hash64Part } from "./pcg32.js";
export { pluginManifestSchema, registerPluginOptionsSchema, registeredPluginSchema, pluginDiagnosticsSchema } from "./plugin.js";
export type { PluginManifest, RegisterPluginOptions, RegisteredPlugin, PluginDiagnostics } from "./plugin.js";
