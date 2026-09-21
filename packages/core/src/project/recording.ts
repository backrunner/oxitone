import {
  automationRecordingEditSchema,
  beatToWire,
  ErrorCode,
  OxitoneError,
  type AutomationRecordingEdit,
} from "@oxitone/protocol";
import { createAutomationNamespace } from "../automation/namespace.js";
import { recordedRanges } from "../automation/recording-source.js";
import { parseAuthoring } from "../authoring-validation.js";
import type { Project } from "./project.js";

/** Add bounded, explicit Playlist overrides; preserve every original automation source and placement. */
export function recordAutomation(project: Project, input: AutomationRecordingEdit): void {
  project.assertMutable();
  const edit = parseAuthoring(automationRecordingEditSchema, input, "project.recordAutomation");
  const target = edit.target;
  const instance =
    target.kind === "instrument"
      ? project.channels[target.index]?.instrumentInstance
      : target.kind === "channelInsert"
        ? project.channels[target.index]?.effectInstances[target.slot]
        : project.mixerChannels[target.index]?.effectInstances[target.slot];
  const config = instance?.config.toSpec();
  if (!instance || config?.pluginId !== target.pluginId || config?.pluginVersion !== target.pluginVersion)
    throw new OxitoneError(ErrorCode.EditTargetMissing, "recording target no longer identifies the same plugin");
  if (!edit.spans.length) return;
  const priority = project.automationLanes.reduce((maximum, lane) => Math.max(maximum, lane.priority ?? 0), 0) + 1;
  if (priority > 0xffffffff)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "automation recording priority exhausted");
  const automation = createAutomationNamespace();
  // Validate the entire take and all curves before allocating entities or mutating Project.
  const plans = edit.parameters
    .map((parameter) => ({
      parameter,
      ranges: recordedRanges(edit.spans, parameter.id).map((range) => {
        beatToWire(range.start);
        const duration = range.end - range.start;
        if (beatToWire(duration).numerator <= 0)
          throw new OxitoneError(ErrorCode.AutomationRange, "recording clip duration cannot be represented");
        return { start: range.start, duration, source: automation.curve(range.points) };
      }),
    }))
    .filter((plan) => plan.ranges.length);
  if (plans.reduce((sum, plan) => sum + plan.ranges.length, 0) > 1024)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "automation recording exceeds 1024 clips");
  const solo = project.tracks.some((track) => track.enabled && track.solo);
  for (const plan of plans) {
    const track = project.addTrack(`${plan.parameter.name} recording`);
    if (solo) track.solo = true;
    for (const range of plan.ranges) {
      const lane = instance
        .param(String(plan.parameter.id))
        .automate(range.source, { playback: "playlist", combine: "replace", priority });
      project.createAutomationClip(lane, track, range.start, range.duration);
    }
  }
}
