import ts from "typescript";
import { pluginConfig } from "@oxitone/core";
import { canonicalEncode, ErrorCode, OxitoneError, type ConfigurationEdit } from "@oxitone/protocol";
import { sourceHash, sourceProgram, expressionAt } from "../syntax/program.js";
import { authoringImport } from "../syntax/imports.js";
import { hasComments, literal, readLiteral } from "../syntax/literals.js";
import type { EvaluatedConfigurationSite } from "../eval/project-evaluation.js";

/** Retain factory execution and imports. Parameter IDs are literal keys, including dots. */
export function writeConfigurationEdit(fileName: string, text: string, site: EvaluatedConfigurationSite, edit: ConfigurationEdit) {
  const { anchor } = site;
  if (sourceHash(text) !== anchor.sourceHash || text.slice(anchor.start, anchor.end) !== anchor.expression) {
    throw new OxitoneError(ErrorCode.SourceChanged, "plugin configuration source changed");
  }
  const original = pluginConfig(site.kind, site.config);
  const config = (edit.kind === "parameters" ? original.withParameters(edit.values) : original.withHost(edit.values)).toSpec();
  const { file, checker } = sourceProgram(fileName, text);
  const expression = expressionAt(file, anchor.start, anchor.end);
  if (!hasComments(expression.getText(file)) && ts.isObjectLiteralExpression(expression)) {
    try {
      if (canonicalEncode(readLiteral(expression)) === canonicalEncode(site.config)) {
        const printed = ts.createPrinter().printNode(ts.EmitHint.Expression, literal(config), file);
        return { text: text.slice(0, anchor.start) + printed + text.slice(anchor.end), config };
      }
    } catch { /* Computed properties, getters and spreads remain normal expressions. */ }
  }
  const imported = authoringImport(file, checker, expression, "pluginConfig");
  const printer = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed });
  const print = (node: ts.Expression) => printer.printNode(ts.EmitHint.Expression, node, file);
  const method = edit.kind === "parameters" ? "withParameters" : "withHost";
  let values = { ...edit.values };
  let base: ts.Expression = ts.factory.createCallExpression(imported.expression, undefined, [literal(site.kind), expression]);
  // Only our imported, pure adapter plus a literal patch is safe to reduce.
  if (!hasComments(expression.getText(file)) && ts.isCallExpression(expression) && expression.arguments.length === 1 && ts.isPropertyAccessExpression(expression.expression)
    && expression.expression.name.text === method && ts.isCallExpression(expression.expression.expression)) {
    const receiver = expression.expression.expression;
    if (receiver.arguments.length === 2 && print(receiver.expression) === print(imported.expression)
      && ts.isStringLiteral(receiver.arguments[0]!) && receiver.arguments[0]!.text === site.kind) {
      try {
        const previous = readLiteral(expression.arguments[0]!) as Record<string, number | boolean>;
        if (previous && !Array.isArray(previous) && typeof previous === "object"
          && Object.entries(previous).every(([key, value]) => (edit.kind === "parameters" ? site.config.parameters[key] : (site.config as Record<string, unknown>)[key]) === value)) {
          values = { ...previous, ...values }; base = receiver;
        }
      } catch { /* Nonliteral arguments retain their evaluation and comments. */ }
    }
  }
  const replacement = print(ts.factory.createCallExpression(ts.factory.createPropertyAccessExpression(base, method), undefined, [literal(values)]));
  let result = text.slice(0, anchor.start) + replacement + text.slice(anchor.end);
  if (imported.patch) result = result.slice(0, imported.patch.at) + imported.patch.text + result.slice(imported.patch.at);
  return { text: result, config };
}
