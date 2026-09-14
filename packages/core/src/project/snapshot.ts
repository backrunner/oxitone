import {
  PROTOCOL_VERSION,
  projectSnapshotSchema,
  type MarkerSpec,
  type ProjectSnapshot,
  type TempoSegment,
} from "@oxitone/protocol";
import type { Pattern } from "../patterns/pattern.js";
import type { Project } from "./project.js";

/** Detached, canonically ordered wire state; native compile validates graph semantics. */
export function snapshotProject(
  project: Project,
  patterns: readonly Pattern[],
  tempoMap: TempoSegment[],
): ProjectSnapshot {
  const byId = <T extends { id: string }>(items: readonly T[]): T[] =>
    [...items].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const clips = byId(project.tracks.flatMap((track) => track.clips));
  const markers: MarkerSpec[] = byId(project.markerSpecs());
  const snapshot = {
    protocolVersion: PROTOCOL_VERSION,
    revision: String(project.revisionBigInt),
    id: project.id,
    ...(project.name !== undefined ? { name: project.name } : {}),
    sampleRate: project.sampleRate,
    blockSize: project.blockSize,
    seed: project.seed,
    tempoMap,
    timeSignatureMap: [...project.timeSignatureMap],
    markers,
    tracks: byId(project.tracks).map((track) => track.toSpec()),
    patterns: byId(patterns).map((pattern) => pattern.toSpec()),
    patternClips: clips.map((clip) => clip.toSpec()),
    sampleClips: byId(project.sampleClips).map((clip) => clip.toSpec()),
    samples: byId(project.samples).map((sample) => sample.toSpec()),
    channels: byId(project.channels).map((channel) => channel.toSpec()),
    automation: byId(project.automationLanes).map((lane) => lane.toSpec()),
    ...(project.automationClips.length === 0
      ? {}
      : { automationClips: byId(project.automationClips).map((clip) => clip.toSpec()) }),
    mixerChannels: byId(project.snapshotMixerChannels()).map((bus) => bus.toSpec()),
  };
  return projectSnapshotSchema.parse(snapshot);
}
