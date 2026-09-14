import { resolve } from "#platform-path";
import { presetSchema, ErrorCode, OxitoneError, type Preset, type ChannelPreset, type InstrumentRef,
  type EffectRef, type SampleRef, type EngineOptions, type RegisterPluginOptions } from "@oxitone/protocol";
import { compile, createEngine, dispose, getPluginInfo, registerPlugin } from "@oxitone/native";
import { Channel, DEFAULT_INSTRUMENT } from "../channels/channel.js";
import { Project } from "../project/project.js";
import { Sample } from "../arrangement/sample.js";
import { parseAuthoring } from "../authoring-validation.js";

export type { Preset, ChannelPreset, InstrumentPreset, EffectPreset } from "@oxitone/protocol";
export interface PresetRuntimeOptions { plugins?: readonly RegisterPluginOptions[]; allowPlugins?: EngineOptions["allowPlugins"]; assetBaseDir?: string | undefined; }
export interface PresetMetadata { name?: string; samples?: readonly Sample[]; }

export function parsePreset(value: unknown): Preset {
  if (typeof value === "object" && value !== null && "formatVersion" in value && value.formatVersion !== "1.0") {
    throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "unsupported preset formatVersion");
  }
  if (typeof value === "object" && value !== null && "abiMajor" in value && value.abiMajor !== 1) {
    throw new OxitoneError(ErrorCode.PluginAbiMismatch, "unsupported preset ABI major");
  }
  return parseAuthoring(presetSchema, value, "preset");
}

/** Capture parameter/state data; only referenced sample descriptors are retained. */
export function createPluginPreset(ref: InstrumentRef | EffectRef, kind: "instrument" | "effect", options: PresetMetadata = {}): Preset {
  return capture({ ...ref, kind, formatVersion: "1.0", abiMajor: 1,
    ...(options.name === undefined ? {} : { name: options.name }) }, options.samples);
}

export function createChannelPreset(channel: Channel, options: PresetMetadata = {}): ChannelPreset {
  return capture({ kind: "channel", formatVersion: "1.0", abiMajor: 1,
    ...(options.name === undefined ? {} : { name: options.name }),
    instrument: channel.instrument, effectChain: channel.effectChain, level: channel.level,
    pan: channel.pan, swing: channel.swing }, options.samples) as ChannelPreset;
}

function references(ref: InstrumentRef | EffectRef): string[] {
  const state = "state" in ref && typeof ref.state === "object" && ref.state !== null ? ref.state : undefined;
  return [...Object.values(ref.resources ?? {}), ...(state && "sampleId" in state ? [String(state.sampleId)] : [])];
}

function plugins(preset: Preset): (InstrumentRef | EffectRef)[] {
  return preset.kind === "channel" ? [preset.instrument, ...preset.effectChain] : [preset];
}

function capture(value: unknown, samples: readonly Sample[] = []): Preset {
  const preset = parsePreset(value);
  const ids = new Set(plugins(preset).flatMap(references));
  preset.samples = samples.filter((sample) => ids.has(sample.id)).map((sample) => sample.toSpec());
  for (const id of ids) if (!preset.samples.some((sample) => sample.id === id)) {
    throw new OxitoneError(ErrorCode.InvalidProject, `preset is missing sample ${id}`);
  }
  return preset;
}

/** Validate against Rust-owned plugin metadata without constructing an audio callback. */
export function validatePreset(value: Preset, options: PresetRuntimeOptions = {}): Preset {
  const preset = parsePreset(value);
  const engine = createEngine({ allowPlugins: options.allowPlugins });
  try {
    for (const plugin of options.plugins ?? []) registerPlugin(engine, plugin);
    for (const [index, ref] of plugins(preset).entries()) {
      const info = getPluginInfo(engine, ref.pluginId, ref.pluginVersion);
      const kind = preset.kind === "channel" ? (index === 0 ? "instrument" : "effect") : preset.kind;
      if (info.kind !== kind) throw new OxitoneError(ErrorCode.PluginManifestMismatch, "preset plugin kind mismatch");
      for (const [id, value] of Object.entries(ref.parameters)) {
        const spec = info.parameters.find((parameter) => parameter.id === id);
        if (!spec || value < spec.min || value > spec.max) throw new OxitoneError(ErrorCode.InvalidProject,
          `invalid preset parameter ${id}`, { details: { path: `parameters.${id}` } });
      }
    }
    const probe = new Project({ seed: 0 }).snapshot();
    probe.samples = preset.samples;
    probe.channels = [{ id: "chn_preset", instrument: preset.kind === "effect" ? DEFAULT_INSTRUMENT : presetInstrument(preset),
      effectChain: preset.kind === "channel" ? preset.effectChain : preset.kind === "effect" ? [presetEffect(preset)] : [],
      level: 1, pan: 0, mixerChannelId: "mix_master" }];
    compile(engine, probe, { assetBaseDir: options.assetBaseDir });
    return preset;
  } finally { dispose(engine); }
}

/** Build and validate a detached candidate before changing any authoring data. */
export async function applyPreset(project: Project, channel: Channel, value: Preset,
  options: { assetBaseDir?: string } = {}): Promise<void> {
  if (!project.channels.includes(channel)) throw new OxitoneError(ErrorCode.InvalidProject, "channel belongs to another project");
  const preset = validatePreset(value, { plugins: project.registeredPlugins, allowPlugins: project.pluginPolicy,
    assetBaseDir: options.assetBaseDir ?? project.assetBaseDir });
  if (preset.kind === "effect") throw new OxitoneError(ErrorCode.InvalidProject, "apply effect presets by replacing an insert with presetEffect()");
  const candidate = Project.fromSnapshot(project.snapshot(), { assetBaseDir: project.assetBaseDir });
  const target = candidate.channels.find((entry) => entry.id === channel.id)!;
  const newSamples: SampleRef[] = [];
  const mapped = new Map<string, string>();
  for (const sample of preset.samples) {
    if (mapped.has(sample.id)) throw new OxitoneError(ErrorCode.InvalidProject, "duplicate preset sample ID");
    const added = candidate.importSampleRef({ ...sample, assetUri: resolve(options.assetBaseDir ?? project.assetBaseDir ?? process.cwd(), sample.assetUri) });
    mapped.set(sample.id, added.id);
    newSamples.push(added.toSpec());
  }
  const remap = <T extends InstrumentRef | EffectRef>(ref: T): T => {
    const next = structuredClone(ref);
    const sampleId = (id: string) => {
      const mappedId = mapped.get(id);
      if (!mappedId) throw new OxitoneError(ErrorCode.InvalidProject, `preset is missing sample ${id}`);
      return mappedId;
    };
    if (next.resources) for (const [key, id] of Object.entries(next.resources)) next.resources[key] = sampleId(id);
    if ("state" in next && typeof next.state === "object" && next.state !== null && "sampleId" in next.state) {
      next.state.sampleId = sampleId(String(next.state.sampleId));
    }
    return next;
  };
  const settings = preset.kind === "channel" ? {
    instrument: remap(preset.instrument), effectChain: preset.effectChain.map(remap),
    level: preset.level, pan: preset.pan, swing: preset.swing,
  } : { instrument: remap(presetInstrument(preset)) };
  target.applySettings(settings);
  const engine = createEngine({ allowPlugins: project.pluginPolicy });
  try {
    for (const plugin of project.registeredPlugins) registerPlugin(engine, plugin);
    compile(engine, candidate.snapshot(), { assetBaseDir: project.assetBaseDir });
  } finally { dispose(engine); }
  project.assertMutable();
  if (project.revisionBigInt + BigInt(newSamples.length + 1) > 0xffff_ffff_ffff_ffffn) {
    throw new OxitoneError(ErrorCode.InvalidProject, "project revision exhausted");
  }
  for (const sample of newSamples) project.importSampleRef(sample, sample.id);
  channel.applySettings(settings);
}

export function presetInstrument(value: Preset): InstrumentRef {
  const preset = parsePreset(value);
  if (preset.kind === "channel") return structuredClone(preset.instrument);
  if (preset.kind !== "instrument") throw new OxitoneError(ErrorCode.InvalidProject, "expected instrument preset");
  const { pluginId, pluginVersion, parameters, resources, state } = preset;
  return { pluginId, pluginVersion, parameters, ...(resources ? { resources } : {}), ...(state === undefined ? {} : { state }) };
}

export function presetEffect(value: Preset): EffectRef {
  const preset = parsePreset(value);
  if (preset.kind !== "effect") throw new OxitoneError(ErrorCode.InvalidProject, "expected effect preset");
  const { pluginId, pluginVersion, parameters, resources, mix, bypass } = preset;
  return { pluginId, pluginVersion, parameters, ...(resources ? { resources } : {}),
    ...(mix === undefined ? {} : { mix }), ...(bypass === undefined ? {} : { bypass }) };
}
