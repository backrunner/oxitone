import { Pattern } from "@oxitone/core";
import type { ConfigurationSite, InstrumentRef, EffectRef } from "@oxitone/protocol";
import { beatFromWire, beatToWire, canonicalEncode, ErrorCode, OxitoneError, type PatternSourceDocument, type ProjectSnapshot, type AutomationSourceSpec, type PreviewSnapshotFrame } from "@oxitone/protocol";

function musicalProject(snapshot: ProjectSnapshot, replacement?: { id: string; source: PatternSourceDocument; placement?: string }): unknown {
  const result = structuredClone(snapshot);
  result.revision = "0";
  const patterns = new Map(result.patterns.map(pattern => [pattern.id, pattern]));
  if (replacement && !replacement.placement) patterns.set(replacement.id, { ...Pattern.fromSource(replacement.source).toSpec(), id: replacement.id });
  if (replacement && !replacement.placement) for (const pattern of patterns.values()) {
    if (pattern.parts?.some(part => part.patternId === replacement.id)) {
      pattern.lengthBeats = beatToWire(Math.max(beatFromWire(pattern.lengthBeats), Pattern.fromSource(replacement.source).lengthBeats));
    }
  }
  const expand = (pattern: ProjectSnapshot["patterns"][number]): unknown => ({
    ...pattern, id: undefined, ...(pattern.parts ? { parts: pattern.parts.map(part => ({
      channelId: part.channelId, pattern: expand(patterns.get(part.patternId)!)
    })) } : {})
  });
  const placed = new Set([...result.patternClips.map(clip => clip.patternId), ...[...patterns.values()].flatMap(pattern => pattern.parts?.map(part => part.patternId) ?? [])]);
  const unplacedPatterns = [...patterns].filter(([id]) => !placed.has(id)).map(([, pattern]) => {
    const value = JSON.parse(JSON.stringify(expand(pattern))); return canonicalEncode(value);
  }).sort();
  // Pattern allocation IDs can change as expressions derive new values. Clip membership remains explicit.
  return JSON.parse(JSON.stringify({ ...result, unplacedPatterns, patterns: undefined, patternClips: result.patternClips.map(clip => {
    const pattern = replacement?.placement === clip.id ? Pattern.fromSource(replacement.source).toSpec() : patterns.get(clip.patternId);
    if (!pattern) throw new OxitoneError(ErrorCode.InvalidProject, "clip references a missing Pattern");
    return { ...clip, patternId: undefined, pattern: expand(pattern) };
  }) }));
}
export function assertProjectNoteEdit(before: ProjectSnapshot, after: ProjectSnapshot, patternId: string, source: PatternSourceDocument, placement?: string): void {
  if (placement && !before.patternClips.some(clip => clip.id === placement && clip.patternId === patternId)) {
    throw new OxitoneError(ErrorCode.EditTargetMissing, "selected placement no longer references this Pattern");
  }
  if (canonicalEncode(musicalProject(before, { id: patternId, source, ...(placement ? { placement } : {}) })) !== canonicalEncode(musicalProject(after))) {
    throw new OxitoneError(ErrorCode.EditScopeConflict, "candidate changed music outside the selected edit scope");
  }
}
export function assertProjectAutomationEdit(before: ProjectSnapshot, after: ProjectSnapshot, lanes: readonly string[], source: AutomationSourceSpec): void {
  const expected = structuredClone(before);
  for (const lane of expected.automation) if (lanes.includes(lane.id)) lane.source = structuredClone(source);
  if (canonicalEncode(musicalProject(expected)) !== canonicalEncode(musicalProject(after))) {
    throw new OxitoneError(ErrorCode.EditScopeConflict, "automation edit changed data outside the selected source and lanes");
  }
}
export function assertFrameConfiguration(before: PreviewSnapshotFrame, after: PreviewSnapshotFrame): void {
  for (const field of ["assetBaseDir", "plugins", "allowPlugins", "pluginUis"] as const) {
    if (JSON.stringify(before[field]) !== JSON.stringify(after[field])) throw new OxitoneError(ErrorCode.EditScopeConflict, "semantic edit changed project plugin configuration");
  }
}

export function assertProjectConfigurationEdit(before: ProjectSnapshot, after: ProjectSnapshot, usages: ConfigurationSite["usages"], config: InstrumentRef | EffectRef): void {
  const expected = structuredClone(before);
  for (const usage of usages) {
    const replace = <T extends InstrumentRef | EffectRef>(previous: T): T => ({ ...structuredClone(config), ...(previous.instanceId ? { instanceId: previous.instanceId } : {}) }) as T;
    if (usage.kind === "busInsert") {
      const bus = expected.mixerChannels.find(bus => bus.id === usage.owner)!;
      bus.inserts[usage.index] = replace(bus.inserts[usage.index]!);
    }
    else {
      const channel = expected.channels.find(channel => channel.id === usage.owner)!;
      if (usage.kind === "instrument") channel.instrument = replace(channel.instrument);
      else channel.effectChain[usage.index] = replace(channel.effectChain[usage.index]!);
    }
  }
  assertProjectEquivalent(expected, after);
}
export function assertProjectEquivalent(before: ProjectSnapshot, after: ProjectSnapshot): void {
  if (canonicalEncode(musicalProject(before)) !== canonicalEncode(musicalProject(after))) {
    throw new OxitoneError(ErrorCode.EditScopeConflict, "configuration edit changed another instance, binding, or project setting");
  }
}
