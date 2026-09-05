export { Project } from "./project.js";
export type { Marker, ProjectOptions } from "./project.js";
export { Session } from "./session.js";
export type { TransportPosition } from "./session.js";
export { Track, SampleClipDraft } from "./track.js";
export { PatternClip, PatternClipDraft } from "./pattern-clip.js";
export { Pattern } from "./pattern.js";
export type { PatternOptions } from "./pattern.js";
export type { NoteInput } from "./note.js";
export { Channel, DEFAULT_INSTRUMENT } from "./channel.js";
export type { ChannelOptions } from "./channel.js";
export { MixerChannel } from "./mixer-channel.js";
export type { MixerChannelOptions, SendOptions } from "./mixer-channel.js";
export { Sample, SampleClip } from "./sample.js";
export type { SampleOptions, SampleClipOptions } from "./sample.js";
export { chord } from "./chord.js";
export type { ChordOptions, ChordQuality, ChordVoicing } from "./chord.js";
export { arp } from "./arp.js";
export type { ArpOptions, ArpOrder, ArpVelocityCurve } from "./arp.js";
export { IdGenerator, ID_PREFIXES } from "./ids.js";
export { TempoMap } from "./tempo-map.js";
export type { TempoCurve, TempoSegmentInput } from "./tempo-map.js";
export { TimeSignatureMap } from "./time-signature.js";
export type { BarBeatPosition } from "./time-signature.js";
export {
  AutomationSource,
  AUTOMATION_MAX_DEPTH,
  AUTOMATION_MAX_NODES,
  validateAutomationSpec,
} from "./automation/source.js";
export { createAutomationNamespace } from "./automation/namespace.js";
export type {
  AutomationNamespace,
  AutomationPointInput,
  GateOptions,
  WaveOptions,
} from "./automation/namespace.js";
export { AutomationLane } from "./automation/lane.js";
export type {
  AutomationCombine,
  AutomationLaneOptions,
  AutomationLaneTarget,
  AutomationLoopInput,
} from "./automation/lane.js";
