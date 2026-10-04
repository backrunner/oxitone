import {
  beatFromWire as beatToNumber,
  beatToWire,
  ErrorCode,
  OxitoneError,
  patternPasteSchema,
  samplePasteSchema,
  automationPasteSchema,
  type ArrangementSingleEdit,
} from "@oxitone/protocol";
import { PatternClip } from "../arrangement/pattern-clip.js";
import { SampleClip } from "../arrangement/sample-clip.js";
import type { Project } from "./project.js";
import { editPreflight } from "./edit-preflight.js";
import { parseAuthoring } from "../authoring-validation.js";

type Paste = Extract<ArrangementSingleEdit, { action: "paste" }>;
export function pasteArrangement(project: Project, edit: Paste): void {
  applyPaste(editPreflight(project), edit);
  applyPaste(project, edit);
}
function require<T>(value: T | undefined): T {
  if (!value) throw new OxitoneError(ErrorCode.EditTargetMissing, "Clipboard resource no longer exists");
  return value;
}
export function applyPaste(project: Project, edit: Paste): void {
  const track = require(project.tracks[edit.track]);
  if (track.tempo !== undefined) throw new OxitoneError(ErrorCode.EditNotRepresentable, "Paste requires project tempo");
  if (edit.kind === "pattern") {
    const settings = parseAuthoring(patternPasteSchema, edit.settings, "arrange.paste.pattern");
    const pattern = require(project.patterns[edit.resource]);
    for (const index of edit.channels ?? []) track.use(require(project.channels[index]));
    const clip = project.createPatternClip(track, pattern, edit.startBeat);
    const copy = PatternClip.fromSpec(project, track, pattern, {
      ...settings,
      id: clip.id,
      patternId: pattern.id,
      trackId: track.id,
      startBeat: beatToWire(edit.startBeat),
      ...(settings.lastBeat === undefined
        ? {}
        : {
            lastBeat: beatToWire(beatToNumber(settings.lastBeat) + edit.startBeat - beatToNumber(settings.startBeat)),
          }),
    });
    track.detachClip(clip);
    track.attachClip(copy);
  } else if (edit.kind === "sample") {
    const settings = parseAuthoring(samplePasteSchema, edit.settings, "arrange.paste.sample");
    const sample = require(project.samples[edit.resource]);
    const clip = project.createSampleClip(track, sample, project.beatsToBarBeat(edit.startBeat), {});
    const copy = SampleClip.fromSpec(project, track, sample, {
      ...settings,
      id: clip.id,
      sampleId: sample.id,
      trackId: track.id,
      startBeat: beatToWire(edit.startBeat),
    });
    track.detachSampleClip(clip);
    track.attachSampleClip(copy);
  } else {
    const settings = parseAuthoring(automationPasteSchema, edit.settings, "arrange.paste.automation");
    const clip = project.createAutomationClip(
      require(project.automationLanes[edit.resource]),
      track,
      edit.startBeat,
      settings.durationBeats === undefined ? undefined : beatToNumber(settings.durationBeats),
    );
    if (settings.enabled !== undefined) clip.enabled = settings.enabled;
  }
}
