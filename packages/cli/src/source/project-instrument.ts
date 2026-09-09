import ts from "typescript";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { sourceHash, sourceProgram } from "./syntax.js";
import type { ExpressionAnchor } from "./pattern-writer.js";
import { captureImport } from "./project-capture-module.js";

export interface SourceSite { handle: string; fileName: string; anchor: ExpressionAnchor; label: string; scope: "definition" | "reference" }

/** Only complete value boundaries: never wrap callees, receivers, lvalues, optional chains or direct eval. */
export function instrumentProjectSource(fileName: string, text: string, key: string, offset: number): { text: string; sites: SourceSite[] } {
  const { file } = sourceProgram(fileName, text);
  const hash = sourceHash(text);
  const sites: SourceSite[] = [];
  const patches: { at: number; text: string; close: boolean }[] = [];
  const imported = captureImport(file, key);
  const visit = (node: ts.Node): void => {
    let expression: ts.Expression | undefined;
    let label = "reference", scope: SourceSite["scope"] = "reference";
    if (ts.isVariableDeclaration(node) && node.initializer && ts.isIdentifier(node.name)) {
      expression = node.initializer; label = node.name.text; scope = "definition";
    } else if (ts.isReturnStatement(node)) { expression = node.expression; label = "return"; }
    else if (ts.isPropertyAssignment(node)) { expression = node.initializer; label = node.name.getText(file).slice(0, 120); }
    else if (ts.isExpressionStatement(node) && !ts.isStringLiteral(node.expression)) { expression = node.expression; label = "statement"; }
    else if (ts.isExportAssignment(node) && !node.isExportEquals) { expression = node.expression; label = "default export"; }
    else if (ts.isArrayLiteralExpression(node)) {
      for (const item of node.elements) if (!ts.isSpreadElement(item) && !ts.isOmittedExpression(item)) add(item, "array item", "reference");
    }
    else if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
      // Arguments are isolated local use sites, even when the factory implementation lives in npm.
      for (const arg of node.arguments ?? []) if (!ts.isSpreadElement(arg)) add(arg, node.expression.getText(file).slice(0, 120), "reference");
    }
    if (expression) add(expression, label, scope);
    ts.forEachChild(node, visit);
  };
  const add = (expression: ts.Expression, label: string, scope: SourceSite["scope"]) => {
    // Functions are not Pattern values, and wrapping anonymous definitions alters inferred names.
    if (ts.isArrowFunction(expression) || ts.isFunctionExpression(expression) || ts.isClassExpression(expression)) return;
    const start = expression.getStart(file), end = expression.end;
    if (sites.some(site => site.anchor.start === start && site.anchor.end === end)) return;
    if (offset + sites.length >= 4096) throw new OxitoneError(ErrorCode.BudgetExceeded, "project exceeds 4096 source boundaries");
    const handle = String(offset + sites.length);
    sites.push({ handle, fileName, anchor: { sourceHash: hash, start, end, expression: text.slice(start, end) }, label, scope });
    patches.push({ at: start, text: `${imported.binding}(${JSON.stringify(handle)},(`, close: false }, { at: end, text: "))", close: true });
  };
  visit(file);
  if (sites.length) patches.push({ at: imported.at, text: imported.text, close: false });
  // Compute the file hash once; AST anchors are independently checked again before any write.
  patches.sort((a, b) => b.at - a.at || Number(a.close) - Number(b.close));
  let result = text;
  for (const patch of patches) result = result.slice(0, patch.at) + patch.text + result.slice(patch.at);
  return { text: result, sites };
}
