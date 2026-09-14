export { ErrorCode, ERROR_CODES, OxitoneError } from "./base/errors.js";
export type { OxitoneErrorCode, OxitoneErrorDetails } from "./base/errors.js";
export { PROTOCOL_VERSION, PROTOCOL_MAJOR, PROTOCOL_MINOR, checkProtocolVersion } from "./base/version.js";
export {
  beatWireSchema,
  beatToWire,
  beatFromWire,
  rationalFromF64,
  BEAT_MAX_DENOMINATOR,
  BEAT_MAX_NUMERATOR,
} from "./base/beat.js";
export type { Beat, BeatWire } from "./base/beat.js";
export {
  entityIdSchema,
  pitchSchema,
  frameWireSchema,
  frameToWire,
  frameFromWire,
  timecodeSchema,
  ID_PREFIXES,
} from "./base/primitives.js";
export type { EntityId, Pitch, FrameWire, Timecode } from "./base/primitives.js";
export {
  inspectSampleRequestSchema,
  sampleInfoSchema,
  cacheSampleRequestSchema,
  cachedSampleInfoSchema,
} from "./authoring/sample-info.js";
export type {
  InspectSampleRequest,
  SampleInfo,
  CacheSampleRequest,
  CachedSampleInfo,
} from "./authoring/sample-info.js";
export { sampleProvenanceSchema } from "./authoring/sample-provenance.js";
export type { SampleProvenance } from "./authoring/sample-provenance.js";
export { wavetableOptionsSchema, samplerOptionsSchema, slicerStateSchema } from "./authoring/instruments.js";
export { modulationSources, modulationTargets } from "./authoring/synth-modulation.js";
export type {
  EnvelopeOptions,
  OscillatorOptions,
  WavetableOptions,
  SamplerOptions,
  SlicerState,
} from "./authoring/instruments.js";
export { curveSchema, automationPointSchema, CURVE_KINDS } from "./authoring/curve.js";
export type { Curve, CurveKind, AutomationPoint } from "./authoring/curve.js";
export { parameterSpecSchema } from "./authoring/parameter.js";
export type { ParameterSpec } from "./authoring/parameter.js";
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
} from "./authoring/automation-source.js";
export type { AutomationSourceSpec, ChanceOptions, WaveKind } from "./authoring/automation-source.js";
export {
  tempoSegmentSchema,
  timeSignatureSegmentSchema,
  loopSpecSchema,
  markerSpecSchema,
  fadeSpecSchema,
} from "./authoring/timeline.js";
export type { TempoSegment, TimeSignatureSegment, LoopSpec, MarkerSpec, FadeSpec } from "./authoring/timeline.js";
export {
  trackSpecSchema,
  sampleEditSpecSchema,
  sampleRefSchema,
  instrumentRefSchema,
  effectRefSchema,
} from "./authoring/refs.js";
export type { TrackSpec, SampleEditSpec, SampleRef, InstrumentRef, EffectRef } from "./authoring/refs.js";
export {
  noteSpecSchema,
  patternSpecSchema,
  patternClipSpecSchema,
  sampleClipSpecSchema,
  channelSpecSchema,
  sendSpecSchema,
  mixerChannelSpecSchema,
  automationLaneSpecSchema,
  automationClipSpecSchema,
} from "./authoring/specs.js";
export type {
  NoteSpec,
  PatternSpec,
  PatternClipSpec,
  SampleClipSpec,
  ChannelSpec,
  SendSpec,
  MixerChannelSpec,
  AutomationLaneSpec,
  AutomationClipSpec,
} from "./authoring/specs.js";
export { projectSnapshotSchema, decodeProjectSnapshot, encodeProjectSnapshot } from "./engine/snapshot.js";
export type { ProjectSnapshot } from "./engine/snapshot.js";
export {
  nativeCommandSchema,
  nativeEventSchema,
  transportCommandSchema,
  transportStateSchema,
  NATIVE_EVENT_TYPES,
  TRANSPORT_STATES,
} from "./engine/commands.js";
export type { NativeCommand, NativeEvent, TransportCommand, TransportState } from "./engine/commands.js";
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
} from "./engine/options.js";
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
} from "./engine/options.js";
export { canonicalize, canonicalEncode } from "./base/canonical.js";
export { PROJECT_FORMAT_VERSION, projectFileSchema, type ProjectFile } from "./document/project-file.js";
export { compileOptionsSchema, type CompileOptions } from "./engine/options.js";
export { beatDurationQuerySchema, beatDurationResultSchema } from "./authoring/timing.js";
export type { BeatDurationQuery } from "./authoring/timing.js";
export { Pcg32, PCG32_MULTIPLIER, PCG32_INCREMENT, hash64, hash64Input } from "./base/pcg32.js";
export type { Hash64Part } from "./base/pcg32.js";
export {
  pluginManifestSchema,
  registerPluginOptionsSchema,
  registeredPluginSchema,
  pluginDiagnosticsSchema,
} from "./plugins/plugin.js";
export type {
  PluginManifest,
  RegisterPluginOptions,
  RegisteredPlugin,
  PluginDiagnostics,
  PluginInfo,
} from "./plugins/plugin.js";
export { pluginInfoSchema } from "./plugins/plugin.js";
export { presetSchema } from "./authoring/preset.js";
export { previewFrameSchema, previewResponseSchema, PREVIEW_MAX_FRAME_BYTES } from "./engine/preview.js";
export type { PreviewFrame, PreviewSnapshotFrame, PreviewResponse } from "./engine/preview.js";
export type { Preset, ChannelPreset, InstrumentPreset, EffectPreset } from "./authoring/preset.js";
export { pluginUiManifestSchema, pluginUiControlSchema } from "./plugins/plugin-ui.js";
export type { PluginUiManifest, PluginUiControl } from "./plugins/plugin-ui.js";
export { multisamplerStateSchema, multisamplerOptionsSchema } from "./authoring/multisampler.js";
export type { MultisamplerState, MultisamplerOptions } from "./authoring/multisampler.js";
export { effectParameterSchemas, effectPluginIds } from "./authoring/effects.js";
export type { EffectKind, EffectParameters } from "./authoring/effects.js";
export {
  PATTERN_SOURCE_FORMAT,
  PATTERN_SOURCE_LIMITS,
  sourceNoteSchema,
  noteSelectorSchema,
  noteEditSchema,
  patternSourceNodeSchema,
  patternSourceDocumentSchema,
} from "./authoring/pattern-source.js";
export type {
  SourceNote,
  NoteSelector,
  NoteEdit,
  PatternSourceNode,
  PatternSourceDocument,
} from "./authoring/pattern-source.js";
export * from "./document/source-daw.js";
export * from "./document/arrangement.js";
export * from "./document/project-edit.js";
export * from "./plugins/plugin-catalog.js";
export * from "./document/configuration-source.js";
