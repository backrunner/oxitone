import { beatToWire } from "@oxitone/protocol";
import { PatternClip } from "./pattern-clip.js";
import { SampleClip } from "./sample-clip.js";
import type { Project } from "../project/project.js";
import type { Track } from "./track.js";

/** Copy the complete placement configuration; only identity, membership and position change. */
export function copyClip(project: Project, original: PatternClip | SampleClip, track: Track, startBeat: number): void {
  if (original instanceof PatternClip) {
    for (const id of original.track.channelIds) {
      const channel = project.channels.find((c) => c.id === id);
      if (channel) track.use(channel);
    }
    const allocated = project.createPatternClip(track, original.pattern, startBeat);
    const spec = original.toSpec();
    const copy = PatternClip.fromSpec(project, track, original.pattern, {
      ...spec,
      id: allocated.id,
      trackId: track.id,
      startBeat: beatToWire(startBeat),
      ...(original.lastBeat === undefined
        ? {}
        : { lastBeat: beatToWire(original.lastBeat + startBeat - original.startBeat) }),
    });
    track.detachClip(allocated);
    track.attachClip(copy);
  } else {
    const spec = original.toSpec(),
      sample = project.samples.find((s) => s.id === spec.sampleId)!;
    const allocated = project.createSampleClip(track, sample, project.beatsToBarBeat(startBeat), {});
    const copy = SampleClip.fromSpec(project, track, sample, {
      ...spec,
      id: allocated.id,
      trackId: track.id,
      startBeat: beatToWire(startBeat),
    });
    track.detachSampleClip(allocated);
    track.attachSampleClip(copy);
  }
}
