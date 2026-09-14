import ts from "typescript";
import { dirname, relative } from "node:path";
import { Project } from "@oxitone/core";
import {
  arrangementEditSchema,
  canonicalEncode,
  ErrorCode,
  OxitoneError,
  projectEditSchema,
  type ArrangementEdit,
  type ProjectEdit,
  type RegisterPluginOptions,
} from "@oxitone/protocol";
import type { ProjectEvaluation } from "../eval/project-evaluation.js";
import { expressionAt, sourceHash, sourceProgram } from "../syntax/program.js";

/** Restore authoring order, which can differ from the canonical snapshot's ID order. */
export function restoreProject(before: ProjectEvaluation): Project {
  const snapshot = structuredClone(before.frame.snapshot);
  for (const key of [
    "patterns",
    "tracks",
    "channels",
    "mixerChannels",
    "samples",
    "automation",
    "patternClips",
    "sampleClips",
    "automationClips",
  ] as const) {
    snapshot[key]?.sort(
      (a, b) => before.arrangementOrder[key].indexOf(a.id) - before.arrangementOrder[key].indexOf(b.id),
    );
  }
  return Project.fromSnapshot(snapshot);
}

/** Preserve imports/exports and evaluate the original final Project expression exactly once. */
export function appendProjectEdit(
  before: ProjectEvaluation,
  files: ReadonlyMap<string, string>,
  method: "arrange" | "configure",
  edit: unknown,
  registration?: RegisterPluginOptions,
): Map<string, string> {
  const rank = (label: string) => (label === "default export" ? 0 : label === "return" ? 1 : 2);
  const site = [...before.projectSites].sort((a, b) => rank(a.label) - rank(b.label) || b.anchor.end - a.anchor.end)[0];
  if (!site || rank(site.label) > 1)
    throw new OxitoneError(
      ErrorCode.EditNotRepresentable,
      "Editing requires a captured Project return or default export",
    );
  const text = files.get(site.fileName)!;
  if (sourceHash(text) !== site.anchor.sourceHash)
    throw new OxitoneError(ErrorCode.SourceChanged, "Project source changed");
  const expression = expressionAt(sourceProgram(site.fileName, text).file, site.anchor.start, site.anchor.end);
  const newline = text.includes("\r\n") ? "\r\n" : "\n";
  let base = site.anchor.expression;
  if (
    !registration &&
    ts.isCallExpression(expression) &&
    ts.isPropertyAccessExpression(expression.expression) &&
    expression.expression.name.text === method &&
    expression.arguments.length === 1 &&
    text.slice(expression.expression.expression.end, expression.end) ===
      `.${method}(${expression.arguments[0]!.getText()})`
  ) {
    // JSON only: don't remove comments, evaluation or computed values from authored calls.
    try {
      const previous =
        method === "configure"
          ? projectEditSchema.parse(JSON.parse(expression.arguments[0]!.getText()))
          : arrangementEditSchema.parse(JSON.parse(expression.arguments[0]!.getText()));
      const next = method === "configure" ? projectEditSchema.parse(edit) : arrangementEditSchema.parse(edit);
      if (scalarTarget(previous) !== undefined && scalarTarget(previous) === scalarTarget(next)) {
        let merged =
          "values" in previous && "values" in next
            ? { ...next, values: { ...previous.values, ...next.values } }
            : previous.kind === "track" && next.kind === "track"
              ? {
                  ...next,
                  enabled: next.enabled ?? previous.enabled,
                  mute: next.mute ?? previous.mute,
                  solo: next.solo ?? previous.solo,
                }
              : next;
        if (previous.kind === "effectOrder" && next.kind === "effectOrder") {
          if (previous.order.length !== next.order.length) throw new Error("chain size changed");
          merged = { ...next, order: next.order.map((index) => previous.order[index]!) };
        }
        let receiver: ts.Expression = expression.expression.expression;
        while (ts.isParenthesizedExpression(receiver) && receiver.getText() === `(${receiver.expression.getText()})`)
          receiver = receiver.expression;
        base = receiver.getText();
        edit = merged;
      }
    } catch {
      /* Preserve unrecognized authored expressions. */
    }
  }
  if (registration) {
    const { libraryPath, ...metadata } = registration;
    const path = relative(dirname(site.fileName), libraryPath);
    base = `(${base}).withPluginRegistration({ ...${JSON.stringify(metadata, null, 2)}, libraryPath: import.meta.dirname + ${JSON.stringify("/" + path)} })`;
  }
  const replacement = `(${base}).${method}(${JSON.stringify(edit, null, 2).replaceAll("\n", newline)})`;
  const candidate = new Map(files);
  candidate.set(site.fileName, text.slice(0, site.anchor.start) + replacement + text.slice(site.anchor.end));
  return candidate;
}
function scalarTarget(edit: ProjectEdit | ArrangementEdit): string | undefined {
  if ("action" in edit)
    return ["resize", "fitSample", "enable"].includes(edit.action) && "clip" in edit
      ? `${edit.kind}/${edit.action}/${edit.resource}/${edit.clip}`
      : undefined;
  if (edit.kind === "tempo") return "tempo";
  if (edit.kind === "effectOrder") return `${edit.kind}/${edit.owner}/${edit.index}`;
  if (edit.kind === "channel" || edit.kind === "bus" || edit.kind === "track") return `${edit.kind}/${edit.index}`;
  return undefined;
}
export function writeProjectEdit(before: ProjectEvaluation, files: ReadonlyMap<string, string>, edit: ProjectEdit) {
  const project = restoreProject(before);
  const original = project.snapshot();
  project.configure(edit);
  const expected = project.snapshot();
  if (canonicalEncode({ ...original, revision: 0 }) === canonicalEncode({ ...expected, revision: 0 })) return undefined;
  return { files: appendProjectEdit(before, files, "configure", edit), expected };
}
