import type { Vst3AutomationTake } from "@oxitone/core";
import { ErrorCode, OxitoneError, type AutomationRecordingEdit } from "@oxitone/protocol";
import type { EvaluatedConfigurationSite, ProjectEvaluation } from "../eval/project-evaluation.js";
import { restoreProject } from "./project-edit-writer.js";
import { recordingExpression } from "./recording-expression.js";

/** A single transaction adds editable recording sources; execution IDs never enter source. */
export function writeVst3Recording(
  before: ProjectEvaluation,
  files: ReadonlyMap<string, string>,
  site: EvaluatedConfigurationSite,
  take: Vst3AutomationTake,
) {
  const usage = site.usages[0]!;
  if (site.usages.length !== 1 || usage.handle !== take.target.instanceId)
    throw new OxitoneError(ErrorCode.EditScopeConflict, "recording does not belong to this source usage");
  const kind = usage.kind;
  const order = kind === "busInsert" ? before.arrangementOrder.mixerChannels : before.arrangementOrder.channels;
  const index = order.indexOf(usage.owner);
  if (index < 0) throw new OxitoneError(ErrorCode.EditTargetMissing, "recording owner is missing");
  const registration = before.frame.vst3Plugins.find(
    (plugin) =>
      `vst3.${plugin.source.classId.toLowerCase()}` === site.config.pluginId &&
      `0.0.0+${plugin.metadata.sha256}` === site.config.pluginVersion,
  );
  const edit: AutomationRecordingEdit = {
    target: {
      kind,
      index,
      ...(kind === "instrument" ? {} : { slot: usage.index }),
      pluginId: site.config.pluginId,
      pluginVersion: site.config.pluginVersion,
    } as AutomationRecordingEdit["target"],
    parameters: take.recording.parameterIds.map((id) => ({
      id,
      name: registration?.metadata.parameters.find((parameter) => parameter.id === id)?.name ?? `Parameter ${id}`,
    })),
    spans: take.spans.map((span) => ({ ...span })),
  };
  const project = restoreProject(before);
  project.recordAutomation(edit);
  return { files: recordingExpression(before, files, project, edit.target), expected: project.snapshot() };
}
