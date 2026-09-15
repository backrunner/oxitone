import ts from "typescript";
import { AutomationSource } from "@oxitone/core";
import { canonicalEncode, ErrorCode, OxitoneError, type AutomationRangeEdit } from "@oxitone/protocol";
import { sourceHash, sourceProgram, expressionAt } from "../syntax/program.js";
import { formatSourceExpression, reindentEmitted } from "../syntax/format.js";
import { readLiteral, literal, hasComments } from "../syntax/literals.js";
import type { EvaluatedAutomationSite } from "../eval/project-evaluation.js";

/** A readable source-local range overlay. Never replaces the original generator with sampled data. */
export function writeAutomationRange(
  fileName: string,
  text: string,
  site: EvaluatedAutomationSite,
  edit: AutomationRangeEdit,
) {
  const { anchor } = site;
  if (sourceHash(text) !== anchor.sourceHash || text.slice(anchor.start, anchor.end) !== anchor.expression) {
    throw new OxitoneError(ErrorCode.SourceChanged, "automation source changed since evaluation");
  }
  const points = edit.points;
  const range = {
    start: edit.start,
    end: edit.end,
    ...(edit.fadeBeats === undefined ? {} : { fadeBeats: edit.fadeBeats }),
  };
  const source = new AutomationSource(site.source).replaceRange(range, points).toSpec();
  const { file } = sourceProgram(fileName, text);
  const expression = expressionAt(file, anchor.start, anchor.end);
  let base = expression;
  if (
    !range.fadeBeats &&
    !hasComments(expression.getText(file)) &&
    site.source.kind === "replaceRange" &&
    ts.isCallExpression(expression) &&
    ts.isPropertyAccessExpression(expression.expression) &&
    expression.expression.name.text === "replaceRange" &&
    expression.arguments.length === 2
  ) {
    try {
      const previousRange = readLiteral(expression.arguments[0]!) as { start: number; end: number; fadeBeats?: number };
      const previousPoints = readLiteral(expression.arguments[1]!) as { beat: number; value: number }[];
      const previous = new AutomationSource(site.source.base).replaceRange(previousRange, previousPoints).toSpec();
      if (
        previousRange.start === range.start &&
        previousRange.end === range.end &&
        canonicalEncode(previous) === canonicalEncode(site.source)
      ) {
        base = expression.expression.expression;
      }
    } catch {
      /* Existing non-literal expressions must retain their execution. */
    }
  }
  const printed = ts
    .createPrinter({ newLine: ts.NewLineKind.LineFeed })
    .printNode(
      ts.EmitHint.Expression,
      ts.factory.createCallExpression(
        ts.factory.createPropertyAccessExpression(ts.factory.createParenthesizedExpression(base), "replaceRange"),
        undefined,
        [literal(range), literal(points)],
      ),
      file,
    );
  const indent = text.slice(text.lastIndexOf("\n", anchor.start - 1) + 1, anchor.start).match(/^[\t ]*/)?.[0] ?? "";
  const replacement = reindentEmitted(
    fileName,
    formatSourceExpression(fileName, text, printed),
    text.includes("\r\n") ? "\r\n" : "\n",
    indent,
  );
  return { text: text.slice(0, anchor.start) + replacement + text.slice(anchor.end), source };
}
