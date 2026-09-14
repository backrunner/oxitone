import { realpathSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { projectSnapshotSchema } from "@oxitone/protocol";
import type { Pattern, Track, AutomationLane } from "@oxitone/core";
import { trackModuleReads } from "./module-read-hooks.js";
import { configurationCaptures } from "../plugins/configuration-captures.js";
import { writeEvaluationFailure } from "./evaluation-failure.js";

try {
  const bundle = realpathSync(resolve(process.argv[2]!)), key = process.argv[3]!;
  const captures = new Map<string, { count: number; values: Set<unknown> }>();
  let count = 0;
  Object.defineProperty(globalThis, key, { configurable: true, value(handle: string, value: unknown) {
    if (++count > 800_000) throw new Error("project execution exceeds source capture budget");
    const capture = captures.get(handle) ?? { count: 0, values: new Set() };
    capture.count++; capture.values.add(value); captures.set(handle, capture);
    return value;
  } });
  // Keep npm TS/TSX and CJS resolution support, without transforming the host's compiled JS graph.
  // With the source-only worker this is the already-loaded preloader, so it does not install twice.
  await import(pathToFileURL(createRequire(import.meta.url).resolve("tsx")).href);
  const tracking = trackModuleReads(bundle);
  try {
    const loaded = await import(pathToFileURL(bundle).href);
    const exported = loaded.default ?? loaded.createProject ?? loaded.project;
    const value = await (typeof exported === "function" ? exported() : exported);
    const project = value?.project ?? value;
    if (!project || typeof project.snapshot !== "function") throw new Error("DAW entry must export a Project or a factory returning one");
    const snapshot = projectSnapshotSchema.parse(project.snapshot());
    const patterns: Pattern[] = project.patterns;
    const tracks: Track[] = project.tracks;
    const placements = tracks.flatMap(track => track.clips).flatMap(clip => [...captures]
      .filter(([, capture]) => capture.count === 1 && capture.values.has(clip))
      .map(([handle]) => ({ handle, clipId: clip.id })));
    const sites = [];
    const lanes: AutomationLane[] = project.automationLanes;
    const automationSites = [...captures].filter(([, capture]) => capture.count === 1).flatMap(([handle, capture]) => {
      const matching = lanes.filter(lane => capture.values.has(lane.source));
      return matching.length ? [{ handle, lanes: matching.map(lane => lane.id), source: matching[0]!.source.toSpec() }] : [];
    });
    const laneCaptures = lanes.flatMap(lane => [...captures].filter(([, capture]) => capture.count === 1 && capture.values.has(lane)).map(([handle]) => ({ handle, laneId: lane.id })));
    for (const pattern of patterns) for (const [handle, capture] of captures) {
      // Identity comparison does not invoke any getters on arbitrary captured user values.
      if (pattern.parts.length === 0 && capture.values.has(pattern)) sites.push({ handle, patternId: pattern.id, invocations: capture.count, source: pattern.toSource() });
    }
    const arrangementOrder = { patterns: patterns.map(p => p.id), tracks: tracks.map(t => t.id), channels: project.channels.map((c: {id: string}) => c.id), mixerChannels: project.mixerChannels.map((b: {id: string}) => b.id),
      samples: project.samples.map((s: {id: string}) => s.id), automation: lanes.map(l => l.id), patternClips: tracks.flatMap(t => t.clips).map(c => c.id), sampleClips: project.sampleClips.map((c: {id: string}) => c.id), automationClips: project.automationClips.map((c: {id: string}) => c.id) };
    const projectCaptures = [...captures].filter(([, c]) => c.count === 1 && c.values.has(project)).map(([handle]) => handle);
    const result = JSON.stringify({ snapshot, sites, placements, automationSites, laneCaptures, arrangementOrder, projectCaptures, ...configurationCaptures(project, captures), reads: [...tracking.reads.values()],
      assetBaseDir: resolve(value?.assetBaseDir ?? project.assetBaseDir ?? loaded.__oxitoneSourceDirectory ?? dirname(bundle)),
      plugins: project.registeredPlugins ?? [], allowPlugins: project.pluginPolicy, pluginUis: project.registeredPluginUis ?? [] });
    if (Buffer.byteLength(result) > 64 * 1024 * 1024) throw new Error("project evaluation exceeds 64 MiB");
    writeFileSync(3, result);
  } finally { tracking.close(); delete (globalThis as Record<string, unknown>)[key]; }
} catch (error) { writeEvaluationFailure(error); console.error(error instanceof Error ? error.stack : error); process.exitCode = 1; }
