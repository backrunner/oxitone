import {
  channelSpecSchema,
  ErrorCode,
  OxitoneError,
  type ChannelSpec,
  type EffectRef,
  type EntityId,
  type InstrumentRef,
} from "@oxitone/protocol";
import { parseAuthoring } from "./authoring-validation.js";
import type { AutomationLane, AutomationLaneOptions } from "./automation/lane.js";
import type { AutomationSource } from "./automation/source.js";
import type { Project } from "./project.js";

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

  /** @internal Use `project.addChannel(...)` instead. */
  constructor(
    id: string,
    options: ChannelOptions,
    mixerChannelId: string,
    private readonly project?: Project,
  ) {
    this.spec = parseAuthoring(channelSpecSchema, {
      ...options, id, instrument: options.instrument ?? DEFAULT_INSTRUMENT,
      effectChain: options.effectChain ?? [], level: options.level ?? 1, pan: options.pan ?? 0,
      mixerChannelId: options.mixerChannelId ?? mixerChannelId,
    }, "channel");
    this.project?.requireMixerChannel(this.spec.mixerChannelId);
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

  addEffect(effect: EffectRef): this {
    this.effectChain = [...this.spec.effectChain, effect];
    return this;
  }

  /** Bind a channel, instrument, or `insert.<index>.parameter.<id>` parameter. */
  automate(
    parameterId: string,
    source: AutomationSource,
    options: AutomationLaneOptions = {},
  ): AutomationLane {
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

  private update(patch: Partial<ChannelSpec>): void {
    this.project?.assertMutable();
    this.spec = parseAuthoring(channelSpecSchema, { ...this.spec, ...patch }, "channel");
    this.project?.touch();
  }
}
