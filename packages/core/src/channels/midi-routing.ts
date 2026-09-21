import { ErrorCode, OxitoneError, type ChannelSpec } from "@oxitone/protocol";

/** Structural validation before mutation or restore; native compilation verifies processor capabilities. */
export function validateMidiRouting(channels: readonly ChannelSpec[]): void {
  const ids = new Set(channels.map((channel) => channel.id));
  const edges = new Map<string, Set<string>>();
  for (const channel of channels) {
    const instances = new Set([channel.instrument, ...channel.effectChain].map((ref) => ref.instanceId));
    const targets = new Set<string>();
    for (const [instance, destinations] of Object.entries(channel.midiRoutes ?? {})) {
      if (!instances.has(instance))
        throw new OxitoneError(ErrorCode.EditTargetMissing, "MIDI route belongs to a detached instance");
      if (
        destinations.length === 0 ||
        destinations.length > 256 ||
        new Set(destinations).size !== destinations.length ||
        destinations.some((id) => !ids.has(id) || id === channel.id)
      )
        throw new OxitoneError(ErrorCode.InvalidProject, "invalid MIDI route destination");
      destinations.forEach((id) => targets.add(id));
    }
    edges.set(channel.id, targets);
  }
  const pending = new Map(channels.map((channel) => [channel.id, 0]));
  for (const targets of edges.values()) for (const id of targets) pending.set(id, pending.get(id)! + 1);
  const ready = [...pending].filter(([, count]) => count === 0).map(([id]) => id);
  let visited = 0;
  while (ready.length) {
    const id = ready.pop()!;
    visited++;
    for (const target of edges.get(id)!) {
      const count = pending.get(target)! - 1;
      pending.set(target, count);
      if (count === 0) ready.push(target);
    }
  }
  if (visited !== channels.length) throw new OxitoneError(ErrorCode.InvalidProject, "MIDI routing cycle");
}

export function retainMidiRoutes(spec: ChannelSpec): ChannelSpec["midiRoutes"] {
  if (spec.midiRoutes === undefined) return undefined;
  const ids = new Set([spec.instrument, ...spec.effectChain].map((ref) => ref.instanceId));
  return Object.fromEntries(Object.entries(spec.midiRoutes).filter(([id]) => ids.has(id)));
}
