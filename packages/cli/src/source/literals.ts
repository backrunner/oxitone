import ts from "typescript";

/** Read JSON-shaped TS syntax without evaluating user code or getters. */
export function readLiteral(node: ts.Expression): unknown {
  if (ts.isStringLiteral(node)) return node.text;
  if (ts.isNumericLiteral(node)) return Number(node.text);
  if (node.kind === ts.SyntaxKind.TrueKeyword) return true;
  if (node.kind === ts.SyntaxKind.FalseKeyword) return false;
  if (node.kind === ts.SyntaxKind.NullKeyword) return null;
  if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.MinusToken && ts.isNumericLiteral(node.operand)) return -Number(node.operand.text);
  if (ts.isArrayLiteralExpression(node)) return node.elements.map((element) => readLiteral(element));
  if (ts.isObjectLiteralExpression(node)) {
    const result: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
    for (const property of node.properties) {
      if (!ts.isPropertyAssignment(property) || !(ts.isIdentifier(property.name) || ts.isStringLiteral(property.name))) throw new Error("non-literal property");
      const key = property.name.text;
      if (key === "__proto__" || Object.hasOwn(result, key)) throw new Error("ambiguous literal property");
      result[key] = readLiteral(property.initializer);
    }
    return result;
  }
  throw new Error("expression requires evaluation");
}

export function literal(value: unknown): ts.Expression {
  const f = ts.factory;
  if (value === null) return f.createNull();
  if (typeof value === "number") return value < 0
    ? f.createPrefixUnaryExpression(ts.SyntaxKind.MinusToken, f.createNumericLiteral(-value)) : f.createNumericLiteral(value);
  if (typeof value === "string") return f.createStringLiteral(value);
  if (typeof value === "boolean") return value ? f.createTrue() : f.createFalse();
  if (Array.isArray(value)) return f.createArrayLiteralExpression(value.map(literal), false);
  if (value !== null && typeof value === "object") return f.createObjectLiteralExpression(
    Object.entries(value).filter(([, child]) => child !== undefined).map(([key, child]) => f.createPropertyAssignment(
      /^[A-Za-z_$][\w$]*$/.test(key) && key !== "__proto__" ? key : f.createComputedPropertyName(f.createStringLiteral(key)), literal(child))), false);
  throw new Error("unsupported source literal");
}

export function printOperations(operations: readonly unknown[], file: ts.SourceFile): string {
  const node = ts.factory.createArrayLiteralExpression(operations.map(literal), true);
  return ts.createPrinter({ newLine: ts.NewLineKind.LineFeed }).printNode(ts.EmitHint.Expression, node, file);
}

export function hasComments(text: string): boolean {
  const scanner = ts.createScanner(ts.ScriptTarget.Latest, false, ts.LanguageVariant.Standard, text);
  for (let token = scanner.scan(); token !== ts.SyntaxKind.EndOfFileToken; token = scanner.scan()) {
    if (token === ts.SyntaxKind.SingleLineCommentTrivia || token === ts.SyntaxKind.MultiLineCommentTrivia) return true;
  }
  return false;
}
