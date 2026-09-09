import {
  ErrorCode,
  mixerChannelSpecSchema,
  OxitoneError,
  type EffectRef,
  type EntityId,
  type MixerChannelSpec,
  type SendSpec,
} from "@oxitone/protocol";
import { parseAuthoring } from "./authoring-validation.js";
import type { AutomationLane, AutomationLaneOptions } from "./automation/lane.js";
import type { AutomationSource } from "./automation/source.js";
import type { Project } from "./project.js";
import { ConfigurationSources } from "./configuration-sources.js";
import { PluginInstances } from "./plugin-instances.js";
import type { PluginInstance } from "./plugin-instance.js";

/** Options for `project.addMixerChannel(...)`; sends are added with `send(...)`. */
export interface MixerChannelOptions {
  name?: string;
  level?: number;
  balance?: number;
  masterSendRatio?: number;
  mute?: boolean;
  solo?: boolean;
  inserts?: readonly EffectRef[];
}

export interface SendOptions {
  ratio?: number;
  preFader?: boolean;
  sidechain?: boolean;
}

/** A project-owned mixer bus; `project.master` is the terminal output bus. */
export class MixerChannel {
  private spec: MixerChannelSpec;
  private readonly instances: PluginInstances;
  /** @internal Source identities, excluded from engine snapshots. */
  readonly configurationSources: ConfigurationSources;

  /** @internal Use `project.addMixerChannel(...)` or `project.master`. */
  constructor(private readonly project: Project, id: EntityId, options: MixerChannelOptions = {}) {
    this.spec = parseAuthoring(mixerChannelSpecSchema, {
      ...options, id, level: options.level ?? 1, balance: options.balance ?? 0,
      inserts: options.inserts ?? [], sends: [],
    }, "mixerChannel");
    this.checkMaster(this.spec);
    this.instances = new PluginInstances(this, project);
    this.spec.inserts = this.instances.adopt(this.spec.inserts);
    this.configurationSources = new ConfigurationSources(undefined, options.inserts ?? []);
  }

  get id(): EntityId {
    return this.spec.id;
  }
  get name(): string | undefined {
    return this.spec.name;
  }
  get isMaster(): boolean {
    return this.id === this.project.masterMixerChannelId;
  }

  get level(): number {
    return this.spec.level;
  }
  set level(value: number) {
    this.update({ level: value });
  }

  get balance(): number {
    return this.spec.balance;
  }
  set balance(value: number) {
    this.update({ balance: value });
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

  /** Undefined on Master, which has no outgoing route. */
  get masterSendRatio(): number | undefined {
    return this.isMaster ? undefined : this.spec.masterSendRatio ?? 1;
  }
  set masterSendRatio(value: number) {
    this.update({ masterSendRatio: value });
  }

  /** Ordered insert chain; replace the array to edit or remove inserts. */
  get inserts(): EffectRef[] {
    return structuredClone(this.spec.inserts);
  }
  set inserts(value: readonly EffectRef[]) {
    this.update({ inserts: [...value] });
  }

  get effectInstances(): readonly PluginInstance[] { return this.spec.inserts.map(ref => this.instances.handle(ref, "effect")); }
  reorderEffects(order: readonly PluginInstance[]): void {
    order.forEach(instance => this.instances.require(instance, "effect"));
    if (order.length !== this.spec.inserts.length || new Set(order).size !== order.length) throw new OxitoneError(ErrorCode.EditScopeConflict, "effect order must be a permutation of this owner's instances");
    const sources = order.map(instance => this.configurationSources.chain[this.spec.inserts.findIndex(ref => ref.instanceId === instance.id)]!);
    this.inserts = order.map(instance => this.spec.inserts.find(ref => ref.instanceId === instance.id)!);
    this.configurationSources.chain = sources;
  }
  removeEffect(instance: PluginInstance): void {
    this.instances.require(instance, "effect");
    const sources = this.configurationSources.chain.filter((_, index) => this.spec.inserts[index]!.instanceId !== instance.id);
    this.inserts = this.spec.inserts.filter(ref => ref.instanceId !== instance.id);
    this.configurationSources.chain = sources;
  }
  /** @internal */
  updateInstance(instance: PluginInstance, config: EffectRef): void {
    this.instances.require(instance, "effect");
    const sources = this.configurationSources.chain.map((source, index) => this.spec.inserts[index]!.instanceId === instance.id ? config : source);
    this.inserts = this.spec.inserts.map(previous => previous.instanceId === instance.id ? { ...config, instanceId: instance.id } : previous);
    this.configurationSources.chain = sources;
  }
  addEffect(effect: EffectRef): PluginInstance {
    const sources = [...this.configurationSources.chain, effect];
    const { instanceId: _, ...config } = effect;
    this.inserts = [...this.spec.inserts, config];
    this.configurationSources.chain = sources;
    return this.effectInstances.at(-1)!;
  }

  /** Sends in authoring order (defensive copy). */
  get sends(): readonly SendSpec[] {
    return structuredClone(this.spec.sends);
  }

  /** Add or replace the one send to a destination, including its tap/detector mode. */
  send(destination: MixerChannel, options: SendOptions = {}): this {
    if (this.isMaster || destination.isMaster) {
      throw new OxitoneError(ErrorCode.InvalidProject, "Master cannot be a send source or destination", {
        details: { path: "mixerChannel.sends" },
      });
    }
    if (destination.project !== this.project || !this.project.mixerChannels.includes(destination)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "send destination must belong to this project", {
        details: { path: "mixerChannel.sends.destinationId" },
      });
    }
    const send: SendSpec = { ...options, destinationId: destination.id, ratio: options.ratio ?? 1 };
    const sends = this.sends.map((entry) => ({ ...entry }));
    const index = sends.findIndex((entry) => entry.destinationId === destination.id);
    if (index < 0) sends.push(send);
    else sends[index] = send;
    // Rust validates the complete audio/detector DAG during compile.
    this.update({ sends });
    return this;
  }

  removeSend(destination: MixerChannel): this {
    if (destination.project !== this.project) {
      throw new OxitoneError(ErrorCode.InvalidProject, "send destination must belong to this project", {
        details: { path: "mixerChannel.sends.destinationId" },
      });
    }
    const sends = this.spec.sends.filter((send) => send.destinationId !== destination.id);
    if (sends.length !== this.spec.sends.length) this.update({ sends });
    return this;
  }

  /** Bind a bus, send ratio, or `insert.<index>.mix/bypass/parameter.<id>` target. */
  automate(
    parameterId: string,
    source: AutomationSource,
    options: AutomationLaneOptions = {},
  ): AutomationLane {
    return this.project.addAutomationLane({ entityId: this.id, parameterId }, source, options);
  }

  toSpec(): MixerChannelSpec {
    return structuredClone(this.spec);
  }

  /** @internal Restore detached wire state without recording an authoring mutation. */
  restoreSpec(input: MixerChannelSpec): void {
    const spec = parseAuthoring(mixerChannelSpecSchema, input, "mixerChannel");
    if (spec.id !== this.id) throw new OxitoneError(ErrorCode.InvalidProject, "mixer identity cannot change");
    this.checkMaster(spec);
    spec.inserts = this.instances.restore(spec.inserts);
    this.spec = spec;
    this.configurationSources.update({ inserts: input.inserts });
  }

  private checkMaster(spec: MixerChannelSpec): void {
    if (this.isMaster && (spec.masterSendRatio !== undefined || spec.sends.length > 0)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "Master has no outgoing routes", {
        details: { path: "mixerChannel.masterSendRatio" },
      });
    }
  }

  private update(patch: Partial<MixerChannelSpec>): void {
    this.project.assertMutable();
    const next = parseAuthoring(mixerChannelSpecSchema, { ...this.spec, ...patch }, "mixerChannel");
    this.checkMaster(next);
    next.inserts = this.instances.adopt(next.inserts, this.spec.inserts);
    this.spec = next;
    this.configurationSources.update(patch);
    if (this.isMaster) this.project.materializeMaster();
    this.project.touch();
  }
}
