import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import type { PluginInstance } from "./plugin-instance.js";

interface EffectOwner {
  readonly effectInstances: readonly PluginInstance[];
  reorderEffects(order: readonly PluginInstance[]): void;
}
/** Reorder this expression's complete chain. Automation keeps its original instance objects. */
export function orderEffects<T extends EffectOwner>(owner: T, order: readonly number[]): T {
  const instances = owner.effectInstances;
  if (order.length !== instances.length || new Set(order).size !== order.length || order.some(index => !Number.isInteger(index) || index < 0 || index >= instances.length)) {
    throw new OxitoneError(ErrorCode.EditScopeConflict, "effect order must contain each current position exactly once");
  }
  owner.reorderEffects(order.map(index => instances[index]!));
  return owner;
}
