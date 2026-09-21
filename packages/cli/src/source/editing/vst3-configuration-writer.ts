import ts from "typescript";
import { pluginConfig } from "@oxitone/core";
import { canonicalEncode, ErrorCode, OxitoneError, type Vst3Configuration } from "@oxitone/protocol";
import type { EvaluatedConfigurationSite } from "../eval/project-evaluation.js";
import { expressionAt, sourceHash, sourceProgram } from "../syntax/program.js";
import { authoringImport } from "../syntax/imports.js";
import { hasComments, literal, readLiteral } from "../syntax/literals.js";
import { formatSourceExpression, reindentEmitted } from "../syntax/format.js";

/** Preserve factory execution and the existing instance/automation identity when storing vendor state. */
export function writeVst3Configuration(text: string, site: EvaluatedConfigurationSite, state: Vst3Configuration) {
  const { anchor, fileName } = site;
  if (sourceHash(text) !== anchor.sourceHash || text.slice(anchor.start, anchor.end) !== anchor.expression)
    throw new OxitoneError(ErrorCode.SourceChanged, "VST3 configuration source changed");
  const config = pluginConfig(site.kind, site.config).replaceParameters(state.parameters).withState(state).toSpec();
  if (`vst3.${state.classId.toLowerCase()}` !== config.pluginId || `0.0.0+${state.sha256}` !== config.pluginVersion)
    throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 state belongs to another instance binary/class");
  const { file, checker } = sourceProgram(fileName, text);
  const original = expressionAt(file, anchor.start, anchor.end);
  const imported = authoringImport(file, checker, original, "pluginConfig");
  const printer = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed });
  const print = (expression: ts.Expression) => printer.printNode(ts.EmitHint.Expression, expression, file);
  let base = original;
  // Reduce only our pure imported adapter and literal payloads. Never accumulate old opaque states.
  if (
    ts.isCallExpression(original) &&
    original.arguments.length === 1 &&
    ts.isPropertyAccessExpression(original.expression) &&
    original.expression.name.text === "withState"
  ) {
    const parameters = original.expression.expression;
    if (
      ts.isCallExpression(parameters) &&
      parameters.arguments.length === 1 &&
      ts.isPropertyAccessExpression(parameters.expression) &&
      parameters.expression.name.text === "replaceParameters"
    ) {
      const adapter = parameters.expression.expression;
      if (
        ts.isCallExpression(adapter) &&
        adapter.arguments.length === 2 &&
        print(adapter.expression) === print(imported.expression) &&
        !hasComments(text.slice(adapter.end, original.end))
      ) {
        try {
          if (
            readLiteral(adapter.arguments[0]!) === site.kind &&
            canonicalEncode(readLiteral(original.arguments[0]!)) === canonicalEncode(site.config.state) &&
            canonicalEncode(readLiteral(parameters.arguments[0]!)) ===
              canonicalEncode((site.config.state as Vst3Configuration).parameters)
          )
            base = adapter.arguments[1]!;
        } catch {
          /* Retain computed expressions and authored evaluation. */
        }
      }
    }
  }
  const call = ts.factory.createCallExpression(imported.expression, undefined, [literal(site.kind), base]);
  const expression = `${print(call)}.replaceParameters(${JSON.stringify(state.parameters)}).withState(${JSON.stringify(state)})`;
  const indent = text.slice(text.lastIndexOf("\n", anchor.start - 1) + 1, anchor.start).match(/^[\t ]*/)?.[0] ?? "";
  const emitted = reindentEmitted(
    fileName,
    formatSourceExpression(fileName, text, expression),
    text.includes("\r\n") ? "\r\n" : "\n",
    indent,
  );
  let result = text.slice(0, anchor.start) + emitted + text.slice(anchor.end);
  if (imported.patch)
    result = result.slice(0, imported.patch.at) + imported.patch.text + result.slice(imported.patch.at);
  return { text: result, config };
}
