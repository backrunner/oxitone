export { Project } from "./project/project.js";
export { PluginConfig, pluginConfig } from "./plugins/plugin-config.js";
export { PluginInstance, InstanceParameter } from "./plugins/plugin-instance.js";
export { orderEffects } from "./channels/effect-order.js";
export type { PluginUiManifest, PluginUiControl } from "@oxitone/protocol";
export {
  createPluginPreset,
  createChannelPreset,
  validatePreset,
  applyPreset,
  presetInstrument,
  presetEffect,
} from "./presets/preset.js";
export type {
  Preset,
  ChannelPreset,
  InstrumentPreset,
  EffectPreset,
  PresetMetadata,
  PresetRuntimeOptions,
} from "./presets/preset.js";
export { savePreset, loadPreset } from "#preset-files";
export type { LoadedPreset } from "./presets/files.js";
export type { Marker, ProjectOptions } from "./project/project.js";
export type { ProjectCompileOptions } from "./engine/playback.js";
export { Session } from "./engine/session.js";
export type { TransportPosition, LoopRegion } from "./engine/session.js";
export { Track, SampleClipDraft } from "./arrangement/track.js";
export { PatternClip, PatternClipDraft } from "./arrangement/pattern-clip.js";
export { Pattern } from "./patterns/pattern.js";
export type { PatternOptions } from "./patterns/pattern.js";
export type { SourceEvent, NoteOrigin } from "./source/types.js";
export type { NoteEdit, NoteSelector, PatternSourceDocument } from "@oxitone/protocol";
export type { NoteInput } from "./notes/note.js";
export { Channel, DEFAULT_INSTRUMENT } from "./channels/channel.js";
export type { ChannelOptions } from "./channels/channel.js";
export { MixerChannel } from "./channels/mixer-channel.js";
export type { MixerChannelOptions, SendOptions } from "./channels/mixer-channel.js";
export { Sample, SampleClip } from "./arrangement/sample.js";
export type { SampleOptions, SampleClipOptions } from "./arrangement/sample.js";
export { wavetable, sampler, slicer } from "./instruments/builders.js";
export { multisampler } from "./instruments/multisampler.js";
export type { SampleRegion, MultisamplerOptions } from "./instruments/multisampler.js";
export { softPiano, grandPiano } from "./instruments/piano.js";
export type { PianoBank, PianoSample } from "./instruments/piano.js";
export type {
  EnvelopeOptions,
  OscillatorOptions,
  WavetableOptions,
  SamplerOptions,
  SlicePosition,
  SliceOptions,
  SlicerOptions,
} from "./instruments/builders.js";
export { chord } from "./notes/chord.js";
export type { ChordOptions, ChordQuality, ChordVoicing } from "./notes/chord.js";
export { arp } from "./notes/arp.js";
export type { ArpOptions, ArpOrder, ArpVelocityCurve } from "./notes/arp.js";
export { IdGenerator, ID_PREFIXES } from "./ids.js";
export { TempoMap } from "./timing/tempo-map.js";
export type { TempoCurve, TempoSegmentInput } from "./timing/tempo-map.js";
export { TimeSignatureMap } from "./timing/time-signature.js";
export type { BarBeatPosition } from "./timing/time-signature.js";
export { loadProject, saveProject } from "#project-files";
export type { LoadedProject, SaveProjectOptions } from "./project/files.js";
export {
  AutomationSource,
  AUTOMATION_MAX_DEPTH,
  AUTOMATION_MAX_NODES,
  validateAutomationSpec,
} from "./automation/source.js";
export { createAutomationNamespace } from "./automation/namespace.js";
export type { AutomationNamespace, AutomationPointInput, GateOptions, WaveOptions } from "./automation/namespace.js";
export { AutomationLane } from "./automation/lane.js";
export { AutomationClip } from "./automation/clip.js";
export type {
  AutomationCombine,
  AutomationLaneOptions,
  AutomationLaneTarget,
  AutomationLoopInput,
} from "./automation/lane.js";
export { effect, convolver } from "./channels/effects.js";
export type { EffectOptions } from "./channels/effects.js";
export type { EffectKind, EffectParameters } from "@oxitone/protocol";
