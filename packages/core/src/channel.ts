import {
  ErrorCode,
  OxitoneError,
  type ChannelSpec,
  type EntityId,
  type InstrumentRef,
} from "@oxitone/protocol";
import type { AutomationLane, AutomationLaneOptions } from "./automation/lane.js";
import type { AutomationSource } from "./automation/source.js";
import type { Project } from "./project.js";

/**
 * Placeholder instrument used when a channel is created without an explicit
 * ref. M1 keeps channels structural; full instrument authoring lands in M2
 * through {@link ChannelOptions.instrument}.
 */
export const DEFAULT_INSTRUMENT: InstrumentRef = {
  pluginId: "oxitone.wavetable",
  pluginVersion: "1.0.0",
  parameters: {},
};

/** Options for `project.addChannel(...)`. */
export interface ChannelOptions {
  name?: string;
  instrument?: InstrumentRef;
  level?: number;
  pan?: number;
  mixerChannelId?: EntityId;
}

/**
 * Host for sound generation and (in M2) insert effects. M1 carries the
 * structural fields needed for a complete snapshot: id, name, instrument
 * ref, level, pan, and mixer routing.
 */
export class Channel {
  readonly id: string;
  private readonly instrumentValue: InstrumentRef;
  readonly mixerChannelId: string;
  private readonly project: Project | undefined;
  private readonly channelName?: string;
  private levelValue: number;
  private panValue: number;

  /** @internal Use `project.addChannel(...)` instead. */
  constructor(id: string, options: ChannelOptions, mixerChannelId: string, project?: Project) {
    this.id = id;
    this.instrumentValue = structuredClone(options.instrument ?? DEFAULT_INSTRUMENT);
    this.mixerChannelId = options.mixerChannelId ?? mixerChannelId;
    this.levelValue = options.level ?? 1;
    this.panValue = options.pan ?? 0;
    if (options.name !== undefined) {
      this.channelName = options.name;
    }
    this.level = this.levelValue;
    this.pan = this.panValue;
    this.project = project;
  }

  get instrument(): InstrumentRef {
    return structuredClone(this.instrumentValue);
  }

  get name(): string | undefined {
    return this.channelName;
  }

  /**
   * Convenience for `project.addAutomationLane(...)` bound to this channel
   * (e.g. `channel.automate('level', automation.sine({ periodBeats: 8 }))`).
   */
  automate(
    parameterId: string,
    source: AutomationSource,
    options: AutomationLaneOptions = {},
  ): AutomationLane {
    if (this.project === undefined) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "channel is not attached to a project; use project.addAutomationLane",
        { details: { path: "automation.target.entityId" } },
      );
    }
    return this.project.addAutomationLane({ entityId: this.id, parameterId }, source, options);
  }

  /** Linear gain in 0..2. */
  get level(): number {
    return this.levelValue;
  }

  set level(value: number) {
    if (!Number.isFinite(value) || value < 0 || value > 2) {
      throw new OxitoneError(ErrorCode.InvalidProject, `channel level must be in 0..2, got ${value}`, {
        details: { path: "channel.level" },
      });
    }
    this.levelValue = value;
    this.project?.touch();
  }

  /** Stereo position in -1..1. */
  get pan(): number {
    return this.panValue;
  }

  set pan(value: number) {
    if (!Number.isFinite(value) || value < -1 || value > 1) {
      throw new OxitoneError(ErrorCode.InvalidProject, `channel pan must be in -1..1, got ${value}`, {
        details: { path: "channel.pan" },
      });
    }
    this.panValue = value;
    this.project?.touch();
  }

  /** Wire form; `effectChain` stays empty until M2 effect authoring. */
  toSpec(): ChannelSpec {
    const spec: ChannelSpec = {
      id: this.id,
      instrument: this.instrument,
      effectChain: [],
      level: this.levelValue,
      pan: this.panValue,
      mixerChannelId: this.mixerChannelId,
    };
    if (this.channelName !== undefined) {
      spec.name = this.channelName;
    }
    return spec;
  }
}
