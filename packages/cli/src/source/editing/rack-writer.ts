import ts from "typescript";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { literal } from "../syntax/literals.js";
import { expressionAt, sourceHash, sourceProgram } from "../syntax/program.js";
import type { EvaluatedRackSite } from "../eval/project-evaluation.js";

/** Materialize a declared serial chain only. Routing graphs and opaque DSP are different boundaries. */
export function materializeRack(fileName: string, text: string, site: EvaluatedRackSite) {
  const { anchor } = site;
  if (sourceHash(text) !== anchor.sourceHash || text.slice(anchor.start, anchor.end) !== anchor.expression) {
    throw new OxitoneError(ErrorCode.SourceChanged, "effect chain source changed");
  }
  const { file, checker } = sourceProgram(fileName, text);
  const expression = expressionAt(file, anchor.start, anchor.end);
  const symbol = ts.isIdentifier(expression) ? checker.getSymbolAtLocation(expression) : undefined;
  const bound = symbol?.declarations?.some(declaration => ts.isVariableDeclaration(declaration) || ts.isImportSpecifier(declaration) || ts.isImportClause(declaration) || ts.isParameter(declaration));
  const effects = ts.factory.createArrayLiteralExpression(site.effects.map(literal), true);
  const printed = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed }).printNode(ts.EmitHint.Expression, effects, file);
  const replacement = bound ? printed : `((${anchor.expression}), ${printed})`;
  return { text: text.slice(0, anchor.start) + replacement + text.slice(anchor.end), retainsOriginalEvaluation: !bound };
}
