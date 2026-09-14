import {
  channelSpecSchema,
  ErrorCode,
  OxitoneError,
  type ChannelSpec,
  type EffectRef,
  type EntityId,
  type InstrumentRef,
} from "@oxitone/protocol";
import { parseAuthoring } from "../authoring-validation.js";
import type { AutomationLane, AutomationLaneOptions } from "../automation/lane.js";
import type { AutomationSource } from "../automation/source.js";
import type { Project } from "../project/project.js";
import { ConfigurationSources } from "./configuration-sources.js";
import { PluginInstances } from "../plugins/plugin-instances.js";
import type { PluginInstance } from "../plugins/plugin-instance.js";

/** Built-in wavetable instrument used when no explicit instrument is supplied. */
export const DEFAULT_INSTRUMENT: InstrumentRef = {
  pluginId: "oxitone.wavetable",
  pluginVersion: "1.0.0",
  parameters: {},
};

/** Options for `project.addChannel(...)`. */
export interface ChannelOptions {
  name?: string;
  instrument?: InstrumentRef;
  effectChain?: readonly EffectRef[];
  level?: number;
  pan?: number;
  swing?: number;
  mute?: boolean;
  solo?: boolean;
  mixerChannelId?: EntityId;
}

/** Instrument and ordered insert effects routed to a project mixer bus. */
export class Channel {
  private spec: ChannelSpec;
  private readonly instances: PluginInstances;
  /** @internal Original value identities for the source evaluator; not execution state. */
  readonly configurationSources: ConfigurationSources;

  /** @internal Use `project.addChannel(...)` instead. */
  constructor(
    id: string,
    options: ChannelOptions,
    mixerChannelId: string,
    private readonly project?: Project,
    restored?: ChannelSpec,
  ) {
    this.spec = parseAuthoring(
      channelSpecSchema,
      {
        ...options,
        id,
        instrument: options.instrument ?? DEFAULT_INSTRUMENT,
        effectChain: options.effectChain ?? [],
        level: options.level ?? 1,
        pan: options.pan ?? 0,
        mixerChannelId: options.mixerChannelId ?? mixerChannelId,
      },
      "channel",
    );
    this.project?.requireMixerChannel(this.spec.mixerChannelId);
    this.instances = new PluginInstances(this, project);
    const refs = restored
      ? this.instances.restore([this.spec.instrument, ...this.spec.effectChain])
      : this.instances.adopt([this.spec.instrument, ...this.spec.effectChain]);
    this.spec.instrument = refs[0]!;
    this.spec.effectChain = refs.slice(1);
    this.configurationSources = new ConfigurationSources(options.instrument, options.effectChain ?? []);
  }

  get id(): EntityId {
    return this.spec.id;
  }
  get name(): string | undefined {
    return this.spec.name;
  }
  get instrument(): InstrumentRef {
    return structuredClone(this.spec.instrument);
  }
  set instrument(value: InstrumentRef) {
    this.update({ instrument: value });
  }
  get instrumentInstance(): PluginInstance {
    return this.instances.handle(this.spec.instrument, "instrument");
  }
  get effectInstances(): readonly PluginInstance[] {
    return this.spec.effectChain.map((ref) => this.instances.handle(ref, "effect"));
  }
  reorderEffects(order: readonly PluginInstance[]): void {
    order.forEach((instance) => this.instances.require(instance, "effect"));
    if (order.length !== this.spec.effectChain.length || new Set(order).size !== order.length)
      throw new OxitoneError(
        ErrorCode.EditScopeConflict,
        "effect order must be a permutation of this owner's instances",
      );
    const sources = order.map(
      (instance) =>
        this.configurationSources.chain[this.spec.effectChain.findIndex((ref) => ref.instanceId === instance.id)]!,
    );
    this.effectChain = order.map((instance) => this.spec.effectChain.find((ref) => ref.instanceId === instance.id)!);
    this.configurationSources.chain = sources;
  }
  removeEffect(instance: PluginInstance): void {
    this.instances.require(instance, "effect");
    const sources = this.configurationSources.chain.filter(
      (_, index) => this.spec.effectChain[index]!.instanceId !== instance.id,
    );
    this.effectChain = this.spec.effectChain.filter((ref) => ref.instanceId !== instance.id);
    this.configurationSources.chain = sources;
  }
  /** @internal */
  updateInstance(instance: PluginInstance, config: InstrumentRef | EffectRef): void {
    this.instances.require(instance, instance.kind);
    const ref = { ...config, instanceId: instance.id };
    if (instance.kind === "instrument") this.instrument = ref;
    else {
      const sources = this.configurationSources.chain.map((source, index) =>
        this.spec.effectChain[index]!.instanceId === instance.id ? config : source,
      );
      this.effectChain = this.spec.effectChain.map((previous) =>
        previous.instanceId === instance.id ? ref : previous,
      );
      this.configurationSources.chain = sources;
    }
  }

  get mixerChannelId(): EntityId {
    return this.spec.mixerChannelId;
  }
  set mixerChannelId(value: EntityId) {
    this.project?.requireMixerChannel(value);
    this.update({ mixerChannelId: value });
  }

  /** Linear gain in 0..2. */
  get level(): number {
    return this.spec.level;
  }
  set level(value: number) {
    this.update({ level: value });
  }

  /** Stereo position in -1..1. */
  get pan(): number {
    return this.spec.pan;
  }
  set pan(value: number) {
    this.update({ pan: value });
  }

  get swing(): number {
    return this.spec.swing ?? 0;
  }
  set swing(value: number) {
    this.update({ swing: value });
  }

  get mute(): boolean {
    return this.spec.mute ?? false;
  }
  set mute(value: boolean) {
    this.update({ mute: value });
  }

  get solo(): boolean {
    return this.spec.solo ?? false;
  }
  set solo(value: boolean) {
    this.update({ solo: value });
  }

  /** Ordered inserts; replacing the array also supports edits/removal. */
  get effectChain(): EffectRef[] {
    return structuredClone(this.spec.effectChain);
  }
  set effectChain(value: readonly EffectRef[]) {
    this.update({ effectChain: [...value] });
  }

  addEffect(effect: EffectRef): PluginInstance {
    const sources = [...this.configurationSources.chain, effect];
    const { instanceId: _, ...config } = effect;
    this.effectChain = [...this.spec.effectChain, config];
    this.configurationSources.chain = sources;
    return this.effectInstances.at(-1)!;
  }

  /** Bind a channel, instrument, or `insert.<index>.parameter.<id>` parameter. */
  automate(parameterId: string, source: AutomationSource, options: AutomationLaneOptions = {}): AutomationLane {
    if (this.project === undefined) {
      throw new OxitoneError(ErrorCode.InvalidProject, "channel is not attached to a project", {
        details: { path: "automation.target.entityId" },
      });
    }
    return this.project.addAutomationLane({ entityId: this.id, parameterId }, source, options);
  }

  toSpec(): ChannelSpec {
    return structuredClone(this.spec);
  }

  /** Apply a validated group of authoring settings as one revision. */
  applySettings(settings: Partial<Pick<ChannelSpec, "instrument" | "effectChain" | "level" | "pan" | "swing">>): void {
    this.update(settings);
  }

  private update(patch: Partial<ChannelSpec>): void {
    this.project?.assertMutable();
    const next = parseAuthoring(channelSpecSchema, { ...this.spec, ...patch }, "channel");
    const refs = this.instances.adopt(
      [next.instrument, ...next.effectChain],
      [this.spec.instrument, ...this.spec.effectChain],
    );
    next.instrument = refs[0]!;
    next.effectChain = refs.slice(1);
    this.spec = next;
    this.configurationSources.update(patch);
    this.project?.touch();
  }
}
