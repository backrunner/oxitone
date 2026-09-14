import { createHash } from "node:crypto";
import ts from "typescript";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";

export const sourceHash = (text: string): string => createHash("sha256").update(text).digest("hex");

/** A local syntax/scope program. Import execution resolution belongs to candidate evaluation. */
export function sourceProgram(fileName: string, text: string): { file: ts.SourceFile; checker: ts.TypeChecker } {
  const file = ts.createSourceFile(fileName, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const options: ts.CompilerOptions = { noLib: true, noResolve: true };
  const host = ts.createCompilerHost(options);
  host.getSourceFile = (name) => name === fileName ? file : undefined;
  const program = ts.createProgram([fileName], options, host);
  const diagnostic = program.getSyntacticDiagnostics(file)[0];
  if (diagnostic) throw new OxitoneError(ErrorCode.DraftInvalid, ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"), {
    details: { path: fileName, start: diagnostic.start },
  });
  return { file, checker: program.getTypeChecker() };
}

export function expressionAt(file: ts.SourceFile, start: number, end: number): ts.Expression {
  let expression: ts.Expression | undefined;
  const visit = (node: ts.Node) => {
    if (node.getStart(file) > start || node.end < end) return;
    if (ts.isExpression(node) && node.getStart(file) === start && node.end === end) expression = node;
    ts.forEachChild(node, visit);
  };
  visit(file);
  if (!expression || !isValueBoundary(expression)) throw new OxitoneError(ErrorCode.EditNotRepresentable,
    "source anchor must cover one complete value expression", { details: { path: file.fileName, start, end } });
  return expression;
}

/** Restrict edits to value positions, not declaration names, lvalues, types, or shorthand keys. */
function isValueBoundary(node: ts.Expression): boolean {
  const parent = node.parent;
  if (ts.isVariableDeclaration(parent) || ts.isPropertyAssignment(parent) || ts.isParameter(parent)) return parent.initializer === node;
  if (ts.isExportAssignment(parent)) return !parent.isExportEquals && parent.expression === node;
  if (ts.isReturnStatement(parent)) return parent.expression === node;
  if (ts.isArrowFunction(parent)) return parent.body === node;
  if (ts.isCallExpression(parent) || ts.isNewExpression(parent)) return parent.arguments?.some((argument) => argument === node) ?? false;
  if (ts.isArrayLiteralExpression(parent)) return parent.elements.some((element) => element === node);
  if (ts.isParenthesizedExpression(parent) || ts.isAsExpression(parent) || ts.isSatisfiesExpression(parent) || ts.isNonNullExpression(parent)) return isValueBoundary(parent);
  if (ts.isConditionalExpression(parent)) return parent.whenTrue === node || parent.whenFalse === node;
  if (ts.isBinaryExpression(parent)) return parent.operatorToken.kind === ts.SyntaxKind.EqualsToken && parent.right === node;
  return false;
}
