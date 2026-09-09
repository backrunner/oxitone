import { canonicalEncode, ErrorCode, OxitoneError, type ArrangementEdit, type ProjectSnapshot } from "@oxitone/protocol";
import type { ProjectEvaluation } from "./project-evaluation.js";
import { appendProjectEdit, restoreProject } from "./project-edit-writer.js";

/** Patch the captured final Project value, never an inferred identifier or source ordinal. */
export function writeArrangement(before: ProjectEvaluation, files: ReadonlyMap<string, string>, edit: ArrangementEdit) {
  const project = restoreProject(before);
  const original = project.snapshot();
  project.arrange(edit);
  const expected = project.snapshot();
  if (canonicalEncode({ ...original, revision: 0 }) === canonicalEncode({ ...expected, revision: 0 })) return undefined;
  return { files: appendProjectEdit(before, files, "arrange", edit), expected };
}

/** Newly allocated identities may differ on restore; every existing entity and all music must match. */
export function assertArrangement(before: ProjectSnapshot, expected: ProjectSnapshot, actual: ProjectSnapshot): void {
  const aliases = new Map<string, string>();
  for (const key of ["channels", "patternClips", "sampleClips", "automationClips"] as const) {
    const old = new Set((before[key] ?? []).map(item => item.id));
    const a = (expected[key] ?? []).filter(item => !old.has(item.id));
    const b = (actual[key] ?? []).filter(item => !old.has(item.id));
    if (a.length !== b.length) fail();
    a.forEach((item, i) => {
      aliases.set(item.id, b[i]!.id);
      if ("instrument" in item && "instrument" in b[i]!) {
        const other = b[i] as typeof item;
        if (item.instrument.instanceId && other.instrument.instanceId) aliases.set(item.instrument.instanceId, other.instrument.instanceId);
      }
    });
  }
  const instances = (snapshot: ProjectSnapshot) => [...snapshot.channels.flatMap(c => [c.instrument, ...c.effectChain ?? []]), ...snapshot.mixerChannels.flatMap(b => b.inserts ?? [])];
  const oldInstances = new Set(instances(before).map(ref => ref.instanceId));
  for (const key of ["channels", "mixerChannels"] as const) {
    for (const owner of expected[key]) {
      const other = actual[key].find(item => item.id === (aliases.get(owner.id) ?? owner.id));
      if (!other) fail();
      const refs = (item: typeof owner) => "instrument" in item ? [item.instrument, ...item.effectChain ?? []] : item.inserts ?? [];
      const a = refs(owner), b = refs(other);
      if (a.length !== b.length) fail();
      a.forEach((ref, i) => {
        const aId = ref.instanceId, bId = b[i]!.instanceId;
        if (aId && bId && !oldInstances.has(aId) && !oldInstances.has(bId)) aliases.set(aId, bId);
      });
    }
  }
  const translated = JSON.parse(JSON.stringify(expected), (_key, value: unknown) => typeof value === "string" ? aliases.get(value) ?? value : value) as ProjectSnapshot;
  translated.revision = actual.revision;
  for (const key of ["channels", "patternClips", "sampleClips", "automationClips"] as const) translated[key]?.sort((a, b) => a.id.localeCompare(b.id));
  if (canonicalEncode(translated) !== canonicalEncode(actual)) fail();
}
function fail(): never { throw new OxitoneError(ErrorCode.EditScopeConflict, "Project transaction changed data outside the requested edit"); }
