import {
  beatToWire, frameToWire, ErrorCode, OxitoneError, samplerOptionsSchema,
  slicerStateSchema, wavetableOptionsSchema, type InstrumentRef, type SamplerOptions,
  type BeatWire, type WavetableOptions,
} from "@oxitone/protocol";
import { parseAuthoring } from "./authoring-validation.js";
import { Sample } from "./sample.js";

export type { EnvelopeOptions, OscillatorOptions, SamplerOptions, WavetableOptions } from "@oxitone/protocol";

function ref(name: string, parameters: Record<string, number>): InstrumentRef {
  return { pluginId: `oxitone.${name}`, pluginVersion: "1.0.0", parameters };
}

function flatten(parameters: Record<string, number>, prefix: string, input: object | undefined): void {
  for (const [key, value] of Object.entries(input ?? {})) {
    if (typeof value === "number") parameters[`${prefix}.${key}`] = value;
  }
}

/** Built-in WavetableSynth. Omitted options retain native descriptor defaults. */
export function wavetable(options: WavetableOptions = {}): InstrumentRef {
  const value = parseAuthoring(wavetableOptionsSchema, options, "wavetable");
  const parameters: Record<string, number> = {};
  for (const key of ["oscA", "oscB"] as const) {
    flatten(parameters, key, value[key]);
    const wave = value[key]?.wave;
    if (wave !== undefined) parameters[`${key}.wavetable`] = ["sine", "saw", "square", "triangle"].indexOf(wave);
  }
  flatten(parameters, "amp", value.amp);
  flatten(parameters, "filterEnv", value.filterEnvelope);
  flatten(parameters, "filter", value.filter);
  if (value.filter?.type !== undefined) parameters["filter.type"] = ["lowpass", "highpass", "bandpass"].indexOf(value.filter.type);
  if (value.mix !== undefined) parameters["osc.mix"] = value.mix;
  if (value.voiceMode !== undefined) parameters.voiceMode = ["poly", "mono", "legato"].indexOf(value.voiceMode);
  for (const key of ["glide", "level", "pan"] as const) if (value[key] !== undefined) parameters[key] = value[key];
  return ref("wavetable", parameters);
}

/** Single-sample instrument using an existing Project Sample and native ADSR/loop playback. */
export function sampler(sample: Sample, options: SamplerOptions = {}): InstrumentRef {
  requireSample(sample);
  const value = parseAuthoring(samplerOptionsSchema, options, "sampler");
  const parameters: Record<string, number> = {};
  flatten(parameters, "amp", value.amp);
  for (const key of ["rootKey", "velocitySensitivity", "level", "pan"] as const) if (value[key] !== undefined) parameters[key] = value[key];
  if (value.loop !== undefined) parameters.loop = value.loop === "forward" ? 1 : 0;
  if (value.startSeconds !== undefined) parameters.start = value.startSeconds;
  return { ...ref("sampler", parameters), resources: { sample: sample.id } };
}

export type SlicePosition = { frames: bigint | number; beat?: never } | { beat: number; frames?: never };
export interface SliceOptions {
  start: SlicePosition;
  end?: SlicePosition;
  level?: number;
  pan?: number;
  rate?: number;
  reverse?: boolean;
}
export interface SlicerOptions {
  slices: readonly SliceOptions[] | { grid: number } | { onset: { algorithm: "onset-v1"; sensitivity?: number } };
  triggerNote?: number;
  playMode?: "oneshot" | "gate";
  tempoSync?: "off" | "repitch";
  level?: number;
  pan?: number;
}

function position(value: SlicePosition): { frames: string } | { beat: BeatWire } {
  if (typeof value !== "object" || value === null) throw new OxitoneError(ErrorCode.InvalidProject, "slice position must be an object");
  if ("frames" in value && !("beat" in value)) {
    const frames = typeof value.frames === "bigint" ? value.frames :
      (Number.isSafeInteger(value.frames) ? BigInt(value.frames!) : undefined);
    if (frames === undefined || frames < 0n || frames > 0xffff_ffff_ffff_ffffn) {
      throw new OxitoneError(ErrorCode.InvalidProject, "slice frames must be a non-negative u64; use bigint for large values");
    }
    return { frames: frameToWire(frames) };
  }
  if ("beat" in value && !("frames" in value)) return { beat: beatToWire(value.beat!) };
  throw new OxitoneError(ErrorCode.InvalidProject, "slice position needs exactly one of frames or beat");
}

/** Build immutable Slicer state; frame/beat markers refer to the prepared sample. */
export function slicer(sample: Sample, options: SlicerOptions): InstrumentRef {
  requireSample(sample);
  if (typeof options !== "object" || options === null) throw new OxitoneError(ErrorCode.InvalidProject, "slicer options are required");
  const { level, pan, ...stateOptions } = options;
  const slices = Array.isArray(options.slices) ? options.slices.map((slice: SliceOptions) => {
    if (typeof slice !== "object" || slice === null) throw new OxitoneError(ErrorCode.InvalidProject, "slice must be an object");
    return { ...slice, start: position(slice.start), ...(slice.end === undefined ? {} : { end: position(slice.end) }) };
  }) : options.slices;
  const state = parseAuthoring(slicerStateSchema, { ...stateOptions, sampleId: sample.id,
    playMode: options.playMode ?? "oneshot", slices }, "slicer.state");
  const parameters: Record<string, number> = {};
  const values = parseAuthoring(samplerOptionsSchema.pick({ level: true, pan: true }), { level, pan }, "slicer");
  if (values.level !== undefined) parameters.level = values.level;
  if (values.pan !== undefined) parameters.pan = values.pan;
  return { ...ref("slicer", parameters), state };
}

function requireSample(sample: Sample): void {
  if (!(sample instanceof Sample)) throw new OxitoneError(ErrorCode.InvalidProject, "instrument requires a Sample");
}
