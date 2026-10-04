import { Project } from "./project.js";

/** Isolated validation retains builder order, which canonical snapshots intentionally sort. */
export function editPreflight(project: Project): Project {
  const snapshot = project.snapshot();
  const order = {
    tracks: project.tracks,
    channels: project.channels,
    mixerChannels: project.mixerChannels,
    samples: project.samples,
    patterns: project.patterns,
    patternClips: project.tracks.flatMap((track) => track.clips),
    sampleClips: project.sampleClips,
    automation: project.automationLanes,
    automationClips: project.automationClips,
  };
  for (const key of Object.keys(order) as (keyof typeof order)[]) {
    const ids = order[key].map((item) => item.id);
    snapshot[key]?.sort((a, b) => ids.indexOf(a.id) - ids.indexOf(b.id));
  }
  return Project.fromSnapshot(snapshot);
}
