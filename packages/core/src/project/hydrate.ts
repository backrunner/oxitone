import type { ProjectSnapshot } from "@oxitone/protocol";
import { AutomationLane } from "../automation/lane.js";
import { AutomationClip } from "../automation/clip.js";
import { Channel, type ChannelOptions } from "../channels/channel.js";
import { Pattern } from "../patterns/pattern.js";
import { PatternClip } from "../arrangement/pattern-clip.js";
import type { Project } from "./project.js";
import { Sample, SampleClip } from "../arrangement/sample.js";
import { Track } from "../arrangement/track.js";

/** Restore after bus identities and clocks; membership has already been checked. */
export function restoreEntities(project: Project, snapshot: ProjectSnapshot) {
  const channels = snapshot.channels.map((spec) => {
    const options: ChannelOptions = { instrument: spec.instrument, effectChain: spec.effectChain,
      level: spec.level, pan: spec.pan, mixerChannelId: spec.mixerChannelId,
      ...(spec.name === undefined ? {} : { name: spec.name }), ...(spec.swing === undefined ? {} : { swing: spec.swing }),
      ...(spec.mute === undefined ? {} : { mute: spec.mute }), ...(spec.solo === undefined ? {} : { solo: spec.solo }) };
    return new Channel(spec.id, options, spec.mixerChannelId, project, spec);
  });
  const leaves = new Map(snapshot.patterns.filter(spec => !spec.parts).map(spec => [spec.id, Pattern.fromSpec(spec)]));
  const patterns = new Map(snapshot.patterns.map(spec => [spec.id, spec.parts ? Pattern.fromSpec(spec, leaves) : leaves.get(spec.id)!]));
  const samples = snapshot.samples.map(Sample.fromSpec);
  const sampleById = new Map(samples.map((sample) => [sample.id, sample]));
  const patternClips = new Map(snapshot.patternClips.map((spec) => [spec.id, spec]));
  const sampleClips = new Map(snapshot.sampleClips.map((spec) => [spec.id, spec]));
  const tracks = snapshot.tracks.map((spec) => {
    const track = Track.fromSpec(project, spec);
    for (const id of spec.patternClipIds) {
      const clip = patternClips.get(id)!;
      track.attachClip(PatternClip.fromSpec(project, track, patterns.get(clip.patternId)!, clip));
    }
    for (const id of spec.sampleClipIds) {
      const clip = sampleClips.get(id)!;
      track.attachSampleClip(SampleClip.fromSpec(project, track, sampleById.get(clip.sampleId)!, clip));
    }
    return track;
  });
  return { channels, patterns, samples, tracks, automation: snapshot.automation.map(AutomationLane.fromSpec), automationClips: (snapshot.automationClips ?? []).map(spec => new AutomationClip(project, spec)) };
}
