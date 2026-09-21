import ts from "typescript";
import type { Project } from "@oxitone/core";
import { ErrorCode, OxitoneError, type AutomationRecordingTarget } from "@oxitone/protocol";
import type { ProjectEvaluation } from "../eval/project-evaluation.js";
import { expressionAt, sourceHash, sourceProgram } from "../syntax/program.js";
import { authoringImport } from "../syntax/imports.js";
import { formatSourceExpression, reindentEmitted } from "../syntax/format.js";

/** Emit ordinary captured Source/Lane builders so recordings remain editable in the DAW. */
export function recordingExpression(
  before: ProjectEvaluation,
  files: ReadonlyMap<string, string>,
  project: Project,
  target: AutomationRecordingTarget,
): Map<string, string> {
  const rank = (label: string) => (label === "default export" ? 0 : label === "return" ? 1 : 2);
  const site = [...before.projectSites].sort((a, b) => rank(a.label) - rank(b.label) || b.anchor.end - a.anchor.end)[0];
  if (!site || rank(site.label) > 1)
    throw new OxitoneError(
      ErrorCode.EditNotRepresentable,
      "Recording requires a captured Project return or default export",
    );
  const text = files.get(site.fileName)!;
  if (sourceHash(text) !== site.anchor.sourceHash)
    throw new OxitoneError(ErrorCode.SourceChanged, "Recording source changed");
  const { file, checker } = sourceProgram(site.fileName, text);
  const expression = expressionAt(file, site.anchor.start, site.anchor.end);
  const imported = authoringImport(file, checker, expression, "AutomationSource");
  const projectType = /\.[cm]?tsx?$/.test(site.fileName)
    ? authoringImport(file, checker, expression, "Project")
    : undefined;
  const constructor = ts.createPrinter().printNode(ts.EmitHint.Expression, imported.expression, file);
  const assertion = projectType ? "!" : "";
  const owner =
    (target.kind === "busInsert"
      ? `recordingProject.mixerChannels[${target.index}]`
      : `recordingProject.channels[${target.index}]`) + assertion;
  const instance =
    target.kind === "instrument"
      ? `${owner}.instrumentInstance`
      : `${owner}.effectInstances[${target.slot}]${assertion}`;
  const lines = [`const recordingInstance = ${instance};`];
  // Local variable initializers are independent source boundaries, even for many clips in one take.
  let index = 0;
  for (const track of project.tracks.filter(
    (track) => !before.frame.snapshot.tracks.some((old) => old.id === track.id),
  )) {
    const trackName = `recordingTrack${index}`;
    lines.push(`const ${trackName} = recordingProject.addTrack(${JSON.stringify(track.name)});`);
    if (track.solo) lines.push(`${trackName}.solo = true;`);
    for (const clip of project.automationClips.filter((clip) => clip.trackId === track.id)) {
      const lane = project.automationLanes.find((lane) => lane.id === clip.laneId)!;
      lines.push(`const recordingSource${index} = new ${constructor}(${JSON.stringify(lane.source.toSpec())});`);
      lines.push(
        `const recordingLane${index} = recordingInstance.param(${JSON.stringify(lane.target.parameterId)}).automate(recordingSource${index}, ${JSON.stringify({ playback: "playlist", combine: "replace", priority: lane.priority })});`,
      );
      lines.push(
        `recordingProject.createAutomationClip(recordingLane${index}, ${trackName}, ${clip.startBeat}, ${clip.durationBeats});`,
      );
      index++;
    }
  }
  const annotation = projectType
    ? `: ${ts.createPrinter().printNode(ts.EmitHint.Expression, projectType.expression, file)}`
    : "";
  const emitted = `((recordingProject${annotation}) => {\n${lines.join("\n")}\nreturn recordingProject;\n})(${site.anchor.expression})`;
  const indent =
    text.slice(text.lastIndexOf("\n", site.anchor.start - 1) + 1, site.anchor.start).match(/^[\t ]*/)?.[0] ?? "";
  const replacement = reindentEmitted(
    site.fileName,
    formatSourceExpression(site.fileName, text, emitted),
    text.includes("\r\n") ? "\r\n" : "\n",
    indent,
  );
  let result = text.slice(0, site.anchor.start) + replacement + text.slice(site.anchor.end);
  if (imported.patch)
    result = result.slice(0, imported.patch.at) + imported.patch.text + result.slice(imported.patch.at);
  const output = new Map(files);
  if (projectType?.patch)
    result = result.slice(0, projectType.patch.at) + projectType.patch.text + result.slice(projectType.patch.at);
  output.set(site.fileName, result);
  return output;
}
