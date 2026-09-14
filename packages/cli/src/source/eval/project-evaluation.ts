import { randomUUID } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { build } from "esbuild";
import { Pattern, AutomationSource } from "@oxitone/core";
import {
  ErrorCode,
  OxitoneError,
  previewFrameSchema,
  type PatternSourceDocument,
  type PreviewSnapshotFrame,
  type AutomationSourceSpec,
  type ConfigurationSite,
} from "@oxitone/protocol";
import { bundleCode, projectBuildOptions } from "../../bundle.js";
import { type SourceSite } from "../plugins/project-instrument.js";
import { runEvaluationProcess } from "./evaluation-process.js";
import { captureResolutionReads } from "./evaluation-inputs.js";
import { captureSourceReads, checkSourceReads, type SourceRead } from "../files/read-set.js";
import type { SourceOwnership } from "../files/ownership.js";
import type { RackSite } from "@oxitone/protocol";
import { projectCaptureModule } from "../plugins/project-capture-module.js";
import { projectSourceLoader } from "../files/project-source-loader.js";
import { sourceSpan } from "./source-timing.js";

export interface EvaluatedSite extends SourceSite {
  patternId: string;
  invocations: number;
  source: PatternSourceDocument;
  placements: string[];
}
export interface EvaluatedAutomationSite extends SourceSite {
  lanes: string[];
  source: AutomationSourceSpec;
}
export interface EvaluatedConfigurationSite extends SourceSite {
  kind: ConfigurationSite["kind"];
  config: ConfigurationSite["config"];
  usages: ConfigurationSite["usages"];
}
export interface EvaluatedRackSite extends SourceSite {
  effects: RackSite["effects"];
  usages: RackSite["usages"];
}
export interface EvaluatedEffectOwnerSite extends SourceSite {
  owner: string;
}
export interface ProjectEvaluation {
  frame: PreviewSnapshotFrame;
  sites: EvaluatedSite[];
  automationSites: EvaluatedAutomationSite[];
  configurationSites: EvaluatedConfigurationSite[];
  rackSites: EvaluatedRackSite[];
  effectOwnerSites: EvaluatedEffectOwnerSite[];
  reads: SourceRead[];
  arrangementOrder: NonNullable<import("@oxitone/protocol").DocumentView["arrangementOrder"]>;
  projectSites: SourceSite[];
}

export async function evaluateSourceProject(
  entry: string,
  files: ReadonlyMap<string, string>,
  ownership: SourceOwnership,
  signal: AbortSignal,
  readPaths: readonly string[] = [],
  removed: readonly string[] = [],
): Promise<ProjectEvaluation> {
  const key = `__oxitone_project_${randomUUID().replaceAll("-", "")}`;
  const reads = new Map<string, SourceRead>();
  for (const read of [...(await captureResolutionReads([entry])), ...(await captureSourceReads(readPaths))])
    reads.set(read.path, read);
  const sites: SourceSite[] = [];
  const buildDone = sourceSpan("bundle");
  let result: Awaited<ReturnType<typeof build>>;
  try {
    const buildOptions = projectBuildOptions(
      entry,
      await projectSourceLoader(files, ownership, key, sites, reads, signal, removed),
    );
    buildOptions.plugins!.unshift(projectCaptureModule(key));
    result = await build(buildOptions);
  } finally {
    buildDone();
  }
  signal.throwIfAborted();
  const directory = await mkdtemp(join(tmpdir(), "oxitone-document-project-"));
  try {
    const bundle = join(directory, "project.mjs");
    await writeFile(bundle, bundleCode(result, entry));
    const value = (await runEvaluationProcess(bundle, key, dirname(entry), signal, 10_000, "project-worker")) as {
      sites: { handle: string; patternId: string; invocations: number; source: unknown }[];
      reads: SourceRead[];
      placements: { handle: string; clipId: string }[];
      arrangementOrder: ProjectEvaluation["arrangementOrder"];
      projectCaptures: string[];
      automationSites: { handle: string; lanes: string[]; source: AutomationSourceSpec }[];
      laneCaptures: { handle: string; laneId: string }[];
      configurationSites: Pick<ConfigurationSite, "handle" | "kind" | "config" | "usages">[];
      ownerCaptures: { handle: string; owner: string; instanceId?: string }[];
      rackSites: Pick<RackSite, "handle" | "effects" | "usages">[];
    };
    const frame = previewFrameSchema.parse({
      ...value,
      type: "snapshot",
      protocolVersion: "1.0",
      hash: "0".repeat(64),
    }) as PreviewSnapshotFrame;
    const captured = value.sites.map((site) => {
      const anchor = sites.find((candidate) => candidate.handle === site.handle);
      if (!anchor) throw new OxitoneError(ErrorCode.EditTargetMissing, "unknown captured source boundary");
      const placements =
        anchor.scope === "definition"
          ? frame.snapshot.patternClips.filter((clip) => clip.patternId === site.patternId).map((clip) => clip.id)
          : value.placements
              .filter((placement) => {
                const boundary = sites.find((candidate) => candidate.handle === placement.handle);
                return (
                  boundary?.fileName === anchor.fileName &&
                  boundary.anchor.start <= anchor.anchor.start &&
                  boundary.anchor.end >= anchor.anchor.end &&
                  frame.snapshot.patternClips.some(
                    (clip) => clip.id === placement.clipId && clip.patternId === site.patternId,
                  )
                );
              })
              .map((placement) => placement.clipId);
      return {
        ...anchor,
        patternId: site.patternId,
        invocations: site.invocations,
        source: Pattern.fromSource(site.source).toSource(),
        placements: [...new Set(placements)],
      };
    });
    const automationSites = value.automationSites
      .map((site) => {
        const anchor = sites.find((candidate) => candidate.handle === site.handle);
        if (!anchor) throw new OxitoneError(ErrorCode.EditTargetMissing, "unknown automation boundary");
        const lanes =
          anchor.scope === "definition"
            ? site.lanes
            : value.laneCaptures
                .filter((capture) => {
                  const boundary = sites.find((candidate) => candidate.handle === capture.handle);
                  return (
                    site.lanes.includes(capture.laneId) &&
                    boundary?.fileName === anchor.fileName &&
                    boundary.anchor.start <= anchor.anchor.start &&
                    boundary.anchor.end >= anchor.anchor.end
                  );
                })
                .map((capture) => capture.laneId);
        return { ...anchor, lanes: [...new Set(lanes)], source: new AutomationSource(site.source).toSpec() };
      })
      .filter((site) => site.lanes.length > 0);
    const configurationSites = value.configurationSites
      .map((site) => {
        const anchor = sites.find((candidate) => candidate.handle === site.handle);
        if (!anchor) throw new OxitoneError(ErrorCode.EditTargetMissing, "unknown configuration boundary");
        const owners = value.ownerCaptures.filter((capture) => {
          const boundary = sites.find((candidate) => candidate.handle === capture.handle);
          return (
            boundary?.fileName === anchor.fileName &&
            boundary.anchor.start <= anchor.anchor.start &&
            boundary.anchor.end >= anchor.anchor.end
          );
        });
        // Fluent Project.configure returns the Project, not the mutated Channel/Bus. The original
        // config object is still captured exactly once and matched to its concrete runtime usages.
        const finalProject = value.projectCaptures.some((handle) => {
          const boundary = sites.find((candidate) => candidate.handle === handle);
          return (
            boundary?.fileName === anchor.fileName &&
            boundary.anchor.start <= anchor.anchor.start &&
            boundary.anchor.end >= anchor.anchor.end
          );
        });
        const usages =
          anchor.scope === "definition" || finalProject
            ? site.usages
            : site.usages.filter((usage) =>
                owners.some(
                  (capture) =>
                    capture.owner === usage.owner && (!capture.instanceId || capture.instanceId === usage.handle),
                ),
              );
        return { ...anchor, kind: site.kind, config: site.config, usages };
      })
      .filter((site) => site.usages.length > 0);
    const rackSites = value.rackSites
      .map((site) => {
        const anchor = sites.find((candidate) => candidate.handle === site.handle);
        if (!anchor) throw new OxitoneError(ErrorCode.EditTargetMissing, "unknown rack boundary");
        const owners = value.ownerCaptures
          .filter((capture) => {
            const boundary = sites.find((candidate) => candidate.handle === capture.handle);
            return (
              boundary?.fileName === anchor.fileName &&
              boundary.anchor.start <= anchor.anchor.start &&
              boundary.anchor.end >= anchor.anchor.end
            );
          })
          .map((capture) => capture.owner);
        return {
          ...anchor,
          effects: site.effects,
          usages:
            anchor.scope === "definition" ? site.usages : site.usages.filter((usage) => owners.includes(usage.owner)),
        };
      })
      .filter((site) => site.usages.length > 0);
    for (const read of value.reads) {
      const previous = reads.get(read.path);
      if (previous && previous.sha256 !== read.sha256)
        throw new OxitoneError(ErrorCode.SourceChanged, "project dependency changed during execution");
      reads.set(read.path, read);
    }
    await checkSourceReads([...reads.values()]);
    signal.throwIfAborted();
    const effectOwnerSites = value.ownerCaptures
      .filter((capture) => !capture.instanceId)
      .flatMap((capture) => {
        const anchor = sites.find((site) => site.handle === capture.handle);
        return anchor ? [{ ...anchor, owner: capture.owner }] : [];
      });
    return {
      frame,
      sites: captured,
      automationSites,
      configurationSites,
      rackSites,
      effectOwnerSites,
      reads: [...reads.values()],
      arrangementOrder: value.arrangementOrder,
      projectSites: sites.filter((site) => value.projectCaptures.includes(site.handle)),
    };
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}
