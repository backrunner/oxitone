import { ErrorCode, OxitoneError, type InsertRouting, type MixerChannelSpec } from "@oxitone/protocol";
import type { Project } from "../project/project.js";

/** Validate owner-local identities and project endpoints; the native compiler validates the complete DAG. */
export function validateInsertRouting(project: Project, spec: MixerChannelSpec): void {
  const ids = new Set([project.masterMixerChannelId, ...project.mixerChannels.map((bus) => bus.id)]);
  validateInsertRoutingIds(spec, ids);
}

export function validateInsertRoutingIds(spec: MixerChannelSpec, ids: ReadonlySet<string>): void {
  for (const [instance, routes] of Object.entries(spec.insertRoutes ?? {})) {
    if (!spec.inserts.some((effect) => effect.instanceId === instance))
      throw new OxitoneError(ErrorCode.EditTargetMissing, "insert route belongs to a detached effect");
    for (const [direction, targets] of Object.entries(routes)) {
      for (const target of Object.values(targets ?? {})) {
        if (
          !ids.has(target) ||
          target === spec.id ||
          (direction === "inputs" && target === "mix_master") ||
          (direction === "outputs" && spec.id === "mix_master")
        )
          throw new OxitoneError(ErrorCode.InvalidProject, "invalid insert bus route endpoint");
      }
    }
  }
}

/** Reordering preserves routes; deletion drops only that instance's outgoing/incoming connections. */
export function retainInsertRoutes(spec: MixerChannelSpec): Record<string, InsertRouting> | undefined {
  if (spec.insertRoutes === undefined) return undefined;
  const ids = new Set(spec.inserts.map((effect) => effect.instanceId));
  return Object.fromEntries(Object.entries(spec.insertRoutes).filter(([id]) => ids.has(id)));
}
