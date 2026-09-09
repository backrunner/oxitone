import { ErrorCode, OxitoneError, type EffectRef, type InstrumentRef } from "@oxitone/protocol";
import type { AutomationSource } from "./automation/source.js";
import type { AutomationLaneOptions } from "./automation/lane.js";
import type { Project } from "./project.js";
import { pluginConfig, type PluginConfig } from "./plugin-config.js";

export interface PluginInstanceOwner {
  readonly id: string;
  toSpec(): { instrument?: InstrumentRef; effectChain?: EffectRef[]; inserts?: EffectRef[] };
  /** @internal Atomic authoring update preserving this instance's identity. */
  updateInstance(instance: PluginInstance, config: InstrumentRef | EffectRef): void;
}

/** An independent plugin use. Its generated execution reference never belongs in authored TS. */
export class PluginInstance {
  /** @internal Instances are created by Channel/Bus, never with caller-supplied IDs. */
  constructor(readonly id: string, readonly kind: "instrument" | "effect", private readonly owner: PluginInstanceOwner, private readonly project?: Project) {}
  get config(): PluginConfig {
    const spec = this.owner.toSpec();
    const ref = this.kind === "instrument" ? spec.instrument : (spec.effectChain ?? spec.inserts)?.find(ref => ref.instanceId === this.id);
    if (!ref || ref.instanceId !== this.id) throw new OxitoneError(ErrorCode.EditTargetMissing, "plugin instance is no longer attached");
    const { instanceId: _, ...config } = ref;
    return pluginConfig(this.kind, config);
  }
  param(parameterId: string): InstanceParameter { return new InstanceParameter(this, parameterId, "plugin"); }
  get host(): { param: (parameterId: "mix" | "bypass") => InstanceParameter } {
    if (this.kind !== "effect") throw new OxitoneError(ErrorCode.AutomationTargetInvalid, "instrument has no insert host parameters");
    return { param: parameterId => new InstanceParameter(this, parameterId, "effectHost") };
  }
  /** @internal */
  belongsTo(owner: PluginInstanceOwner): boolean { return this.owner === owner; }
  /** @internal */
  set(parameter: string, scope: "plugin" | "effectHost", value: number | boolean): void {
    const config = this.config;
    let next: PluginConfig;
    if (scope === "plugin") {
      if (typeof value !== "number") throw new OxitoneError(ErrorCode.InvalidProject, "plugin parameters use physical numeric values");
      next = config.withParameters({ [parameter]: value });
    } else if (parameter === "mix" && typeof value === "number") next = config.withHost({ mix: value });
    else if (parameter === "bypass" && typeof value === "boolean") next = config.withHost({ bypass: value });
    else throw new OxitoneError(ErrorCode.AutomationTargetInvalid, "invalid insert host setting");
    this.owner.updateInstance(this, next.toSpec());
  }
  /** @internal */
  automate(parameterId: string, scope: "plugin" | "effectHost", source: AutomationSource, options: AutomationLaneOptions) {
    void this.config;
    if (!this.project) throw new OxitoneError(ErrorCode.InvalidProject, "plugin instance is detached from a project");
    return this.project.addAutomationLane({ entityId: this.id, parameterId, scope }, source, options);
  }
}
export class InstanceParameter {
  /** @internal */
  constructor(private readonly instance: PluginInstance, private readonly parameter: string, private readonly scope: "plugin" | "effectHost") {
    if (!parameter) throw new OxitoneError(ErrorCode.AutomationTargetInvalid, "parameter ID is empty");
  }
  set(value: number | boolean): void { this.instance.set(this.parameter, this.scope, value); }
  automate(source: AutomationSource, options: AutomationLaneOptions = {}) { return this.instance.automate(this.parameter, this.scope, source, options); }
}
