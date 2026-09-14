//! Playlist edits mutate authoring builders only; native compilation remains authoritative.
import { arrangementEditSchema, ErrorCode, OxitoneError, type ArrangementEdit } from "@oxitone/protocol";
import type { Project } from "./project.js";
import { copyClip } from "../arrangement/copy.js";

export function arrange(project: Project, input: ArrangementEdit): void {
  const edit = arrangementEditSchema.parse(input);
  const require = <T>(value: T | undefined): T => {
    if (value === undefined) throw new OxitoneError(ErrorCode.EditTargetMissing, "Playlist resource or Track no longer exists");
    return value;
  };
  project.assertMutable();
  const track = "track" in edit ? require(project.tracks[edit.track]) : undefined;
  if (track?.tempo !== undefined) throw new OxitoneError(ErrorCode.EditNotRepresentable, "Playlist editing requires a Track following project tempo");
  if (edit.kind === "automation") {
    const lane = require(project.automationLanes[edit.resource]);
    if (edit.action === "place") {
      project.createAutomationClip(lane, require(project.tracks[edit.track]), edit.startBeat, edit.durationBeats);
    } else {
      const clip = require(project.automationClips[edit.clip]);
      if (clip.laneId !== lane.id) throw new OxitoneError(ErrorCode.EditTargetMissing, "automation clip does not belong to lane");
      if (edit.action === "remove") {
        project.removeAutomationClip(clip);
      } else if (edit.action === "enable") {
        clip.enabled = edit.enabled;
      } else if (edit.action === "resize") {
        clip.durationBeats = edit.durationBeats;
      } else if (edit.action === "duplicate") {
        const copy = project.createAutomationClip(lane, track!, edit.startBeat, clip.durationBeats);
        if (clip.toSpec().enabled !== undefined) copy.enabled = clip.enabled;
      } else {
        clip.relocate(track!, edit.startBeat);
      }
    }
    return;
  }
  if (edit.action !== "place") {
    if (edit.kind === "pattern") {
      const clip = require(project.tracks.flatMap(t => t.clips)[edit.clip]);
      if (clip.pattern !== require(project.patterns[edit.resource])) throw new OxitoneError(ErrorCode.EditTargetMissing, "Pattern clip resource changed");
      if ("track" in edit && clip.track.tempo !== undefined) throw new OxitoneError(ErrorCode.EditNotRepresentable, "Moving or copying a clip requires project tempo at both ends");
      if (edit.action === "remove") clip.track.detachClip(clip);
      else if (edit.action === "enable") clip.enabled(edit.enabled);
      else if (edit.action === "resize") {
        if (clip.track.tempo !== undefined) throw new OxitoneError(ErrorCode.EditNotRepresentable, "Resize requires project tempo");
        if (clip.lastBeat !== undefined) clip.last(project.beatsToBarBeat(clip.startBeat + edit.durationBeats));
        else clip.durationBeats = edit.durationBeats;
      }
      else if (edit.action === "duplicate") copyClip(project, clip, track!, edit.startBeat);
      else if (edit.action === "move") clip.relocate(track!, edit.startBeat);
    } else {
      const clip = require(project.sampleClips[edit.clip]);
      if (clip.toSpec().sampleId !== require(project.samples[edit.resource]).id) throw new OxitoneError(ErrorCode.EditTargetMissing, "Sample clip resource changed");
      if ("track" in edit && clip.track.tempo !== undefined) throw new OxitoneError(ErrorCode.EditNotRepresentable, "Moving or copying a clip requires project tempo at both ends");
      if (edit.action === "remove") clip.track.detachSampleClip(clip);
      else if (edit.action === "enable") clip.enabled = edit.enabled;
      else if (edit.action === "fitSample") {
        if (clip.track.tempo !== undefined) throw new OxitoneError(ErrorCode.EditNotRepresentable, "Fit requires project tempo");
        clip.fitBeats(edit.durationBeats);
      }
      else if (edit.action === "duplicate") copyClip(project, clip, track!, edit.startBeat);
      else if (edit.action === "move") clip.relocate(track!, edit.startBeat);
    }
    project.touch(); return;
  }
  if (edit.kind === "pattern") {
    const pattern = require(project.patterns[edit.resource]);
    const existing = project.tracks.flatMap(t => t.clips).find(c => c.pattern === pattern);
    if (track!.channelIds.length === 0 && existing) {
      for (const id of existing.track.channelIds) {
        const channel = project.channels.find(candidate => candidate.id === id);
        if (channel) track!.use(channel);
      }
    }
    const clip = project.createPatternClip(track!, pattern, edit.startBeat);
    if (edit.durationBeats !== undefined) clip.durationBeats = edit.durationBeats;
  } else {
    const sample = require(project.samples[edit.resource]);
    project.createSampleClip(track!, sample, project.beatsToBarBeat(edit.startBeat), {
      ...(edit.durationBeats !== undefined ? { durationBeats: edit.durationBeats } : {}),
    });
  }
}
