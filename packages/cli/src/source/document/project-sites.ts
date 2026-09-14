import { Pattern } from "@oxitone/core";
import type { DocumentView } from "@oxitone/protocol";
import type { ProjectEvaluation } from "../eval/project-evaluation.js";
import type { SourceSite } from "../plugins/project-instrument.js";

/** Source projection is separate from the document's mutable transaction state. */
export function projectSites(
  accepted: ProjectEvaluation | undefined,
  handle: (site: SourceSite) => string,
): Pick<
  DocumentView,
  "sites" | "effectOwnerSites" | "rackSites" | "configurationSites" | "automationSites" | "arrangementOrder"
> {
  const source = (site: SourceSite) => ({
    handle: handle(site),
    fileName: site.fileName,
    expression: site.anchor.expression,
    label: site.label,
    scope: site.scope,
  });
  return {
    ...(accepted ? { arrangementOrder: accepted.arrangementOrder } : {}),
    sites: (accepted?.sites ?? [])
      .filter((site) => site.invocations === 1)
      .map((site) => {
        const pattern = Pattern.fromSource(site.source);
        return {
          ...source(site),
          patternId: site.patternId,
          start: site.anchor.start,
          end: site.anchor.end,
          invocations: site.invocations,
          references: accepted!.frame.snapshot.patternClips.filter(
            (clip) =>
              clip.patternId === site.patternId ||
              accepted!.frame.snapshot.patterns
                .find((pattern) => pattern.id === clip.patternId)
                ?.parts?.some((part) => part.patternId === site.patternId),
          ).length,
          placements: site.placements,
          outputs: pattern.outputs.map((output) => ({
            note: { ...output.note, tags: output.note.tags ? [...output.note.tags] : undefined },
            select: output.select,
          })),
        };
      }),
    effectOwnerSites: (accepted?.effectOwnerSites ?? []).map((site) => ({ ...source(site), owner: site.owner })),
    rackSites: (accepted?.rackSites ?? []).map((site) => ({
      ...source(site),
      effects: site.effects,
      usages: site.usages,
    })),
    configurationSites: (accepted?.configurationSites ?? []).map((site) => ({
      ...source(site),
      kind: site.kind,
      config: site.config,
      usages: site.usages,
    })),
    automationSites: (accepted?.automationSites ?? []).map((site) => ({
      ...source(site),
      lanes: site.lanes,
      clips: (accepted!.frame.snapshot.automationClips ?? [])
        .filter((clip) => site.lanes.includes(clip.laneId))
        .map((clip) => clip.id),
      source: site.source,
    })),
  };
}
