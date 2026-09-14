import type { Project } from "@oxitone/core";
import { canonicalEncode, type ConfigurationUsage } from "@oxitone/protocol";

type Captures = ReadonlyMap<string, { count: number; values: ReadonlySet<unknown> }>;
function configuration<T extends { instanceId?: string | undefined }>(ref: T): Omit<T, "instanceId"> { const { instanceId: _, ...config } = ref; return config; }
export function configurationCaptures(project: Project, captures: Captures) {
  const owners = [...project.channels, ...project.mixerChannels];
  const references = owners.flatMap(owner => {
    const spec = owner.toSpec(), source = owner.configurationSources;
    const instrument = "instrument" in spec && source.instrument ? [{ input: source.instrument, kind: "instrument" as const, config: configuration(spec.instrument),
      usage: { handle: spec.instrument.instanceId ?? `${owner.id}:instrument`, owner: owner.id, label: owner.name ?? owner.id, kind: "instrument" as const, index: 0 } }] : [];
    const chain = "instrument" in spec ? spec.effectChain : spec.inserts;
    return [...instrument, ...chain.map((config, index) => ({ input: source.chain[index], kind: "effect" as const, config: configuration(config),
      usage: { handle: config.instanceId ?? `${owner.id}:effect:${index}`, owner: owner.id, label: owner.name ?? owner.id,
        kind: "instrument" in spec ? "channelInsert" as const : "busInsert" as const, index } }))];
  });
  const configurationSites = [...captures].filter(([, capture]) => capture.count === 1).flatMap(([handle, capture]) => {
    const matching = references.filter(reference => reference.input && capture.values.has(reference.input));
    // A later mutation can make one source value diverge across consumers. Do not guess its value.
    if (!matching.length || matching.some(ref => ref.kind !== matching[0]!.kind || canonicalEncode(ref.config) !== canonicalEncode(matching[0]!.config))) return [];
    return [{ handle, kind: matching[0]!.kind, config: matching[0]!.config, usages: matching.map(ref => ref.usage) as ConfigurationUsage[] }];
  });
  const ownerCaptures: { handle: string; owner: string; instanceId?: string }[] = owners.flatMap(owner => [...captures].filter(([, capture]) => capture.count === 1 && capture.values.has(owner))
    .map(([handle]) => ({ handle, owner: owner.id })));
  for (const owner of owners) for (const instance of owner.effectInstances) {
    for (const [handle, capture] of captures) if (capture.count === 1 && capture.values.has(instance)) ownerCaptures.push({ handle, owner: owner.id, instanceId: instance.id });
  }
  const rackSites = [...captures].filter(([, capture]) => capture.count === 1).flatMap(([handle, capture]) => {
    const matching = owners.filter(owner => capture.values.has(owner.configurationSources.chain)).map(owner => {
      const spec = owner.toSpec();
      return { effects: ("instrument" in spec ? spec.effectChain : spec.inserts).map(configuration),
        usage: { owner: owner.id, label: owner.name ?? owner.id, kind: "instrument" in spec ? "channel" as const : "bus" as const } };
    });
    if (!matching.length || matching.some(rack => canonicalEncode(rack.effects) !== canonicalEncode(matching[0]!.effects))) return [];
    return [{ handle, effects: matching[0]!.effects, usages: matching.map(rack => rack.usage) }];
  });
  return { configurationSites, ownerCaptures, rackSites };
}
