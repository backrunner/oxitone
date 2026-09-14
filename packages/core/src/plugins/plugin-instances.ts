import { ErrorCode, OxitoneError, hash64, type EffectRef, type InstrumentRef } from "@oxitone/protocol";
import { PluginInstance, type PluginInstanceOwner } from "./plugin-instance.js";
import type { Project } from "../project/project.js";

/** Owner-local allocation is independent of the project's musical entity/random sequence. */
export class PluginInstances {
  private serial = 0;
  private readonly allocated = new Set<string>();
  private readonly handles = new Map<string, PluginInstance>();
  constructor(
    private readonly owner: PluginInstanceOwner,
    private readonly project: Project | undefined,
  ) {}
  adopt<T extends InstrumentRef | EffectRef>(
    refs: readonly T[],
    previous: readonly (InstrumentRef | EffectRef)[] = [],
  ): T[] {
    const attached = new Set(previous.map((ref) => ref.instanceId));
    return this.allocate(
      refs.map((ref) => {
        if (ref.instanceId && attached.has(ref.instanceId)) return ref;
        const { instanceId: _, ...config } = ref;
        return config as T;
      }),
      previous,
    );
  }
  /** Restore execution identities only from a validated snapshot, never from copied authoring config. */
  restore<T extends InstrumentRef | EffectRef>(refs: readonly T[]): T[] {
    return this.allocate(refs, []);
  }
  private allocate<T extends InstrumentRef | EffectRef>(
    refs: readonly T[],
    previous: readonly (InstrumentRef | EffectRef)[],
  ): T[] {
    let serial = this.serial;
    const allocated = new Set(this.allocated);
    for (const ref of refs) if (ref.instanceId) allocated.add(ref.instanceId);
    const next = refs.map((ref) => {
      if (ref.instanceId) return ref;
      let id: string;
      do {
        id = `ins_${hash64(this.owner.id).toString(16)}_${++serial}`;
      } while (allocated.has(id));
      allocated.add(id);
      return { ...ref, instanceId: id };
    });
    this.checkChange(previous, next);
    this.serial = serial;
    for (const id of allocated) this.allocated.add(id);
    return next;
  }
  handle(ref: InstrumentRef | EffectRef, kind: "instrument" | "effect"): PluginInstance {
    if (!ref.instanceId) throw new OxitoneError(ErrorCode.EditTargetMissing, "plugin has no execution identity");
    let instance = this.handles.get(ref.instanceId);
    if (!instance) {
      instance = new PluginInstance(ref.instanceId, kind, this.owner, this.project);
      this.handles.set(ref.instanceId, instance);
    }
    return instance;
  }
  require(instance: PluginInstance, kind: "instrument" | "effect"): void {
    if (!instance.belongsTo(this.owner) || instance.kind !== kind)
      throw new OxitoneError(ErrorCode.EditScopeConflict, "plugin instance belongs to another owner or role");
    void instance.config;
  }
  checkChange(previous: readonly (InstrumentRef | EffectRef)[], next: readonly (InstrumentRef | EffectRef)[]): void {
    const ids = new Set(next.map((ref) => ref.instanceId));
    if (ids.size !== next.length)
      throw new OxitoneError(ErrorCode.InvalidProject, "duplicate plugin instance in owner");
    if (
      next.some((ref) =>
        previous.some(
          (old) =>
            old.instanceId === ref.instanceId &&
            (old.pluginId !== ref.pluginId || old.pluginVersion !== ref.pluginVersion),
        ),
      )
    ) {
      throw new OxitoneError(
        ErrorCode.EditScopeConflict,
        "a replacement plugin needs a new instance and explicit binding migration",
      );
    }
    const structureChanged =
      previous.length !== next.length || previous.some((ref, index) => ref.instanceId !== next[index]?.instanceId);
    if (!structureChanged) return;
    for (const lane of this.project?.automationLanes ?? []) {
      if (
        lane.target.scope &&
        previous.some((ref) => ref.instanceId === lane.target.entityId) &&
        !ids.has(lane.target.entityId)
      ) {
        throw new OxitoneError(
          ErrorCode.EditScopeConflict,
          "remove or explicitly remap automation before deleting/replacing its plugin instance",
        );
      }
      if (!lane.target.scope && lane.target.entityId === this.owner.id) {
        const instrumentOffset = "instrument" in this.owner.toSpec() ? 1 : 0;
        const insert = /^insert\.(\d+)\./.exec(lane.target.parameterId);
        const index = insert ? Number(insert[1]) + instrumentOffset : 0;
        const pluginParameter =
          insert || (instrumentOffset && !["level", "pan", "mute", "swing"].includes(lane.target.parameterId));
        if (pluginParameter && previous[index]?.instanceId !== next[index]?.instanceId) {
          throw new OxitoneError(
            ErrorCode.EditScopeConflict,
            "migrate legacy automation to instance targets before replacing its addressed plugin",
          );
        }
      }
    }
  }
}
