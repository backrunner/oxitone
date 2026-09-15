import ts from "typescript";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { formatSourceStatement } from "./format.js";

export interface ImportPatch {
  readonly at: number;
  readonly text: string;
}
export interface PatternImport {
  readonly expression: ts.Expression;
  readonly patch?: ImportPatch;
}

/** Insert after the exact directive statements, including when other statements share their line. */
export function importPosition(file: ts.SourceFile): number {
  let at = file.statements[0]?.getStart(file) ?? file.text.length;
  let directive = false;
  for (const statement of file.statements) {
    if (!ts.isExpressionStatement(statement) || !ts.isStringLiteral(statement.expression)) break;
    at = statement.end;
    directive = true;
  }
  if (directive) {
    const scanner = ts.createScanner(ts.ScriptTarget.Latest, false, file.languageVariant, file.text, undefined, at);
    for (let token = scanner.scan(); ; token = scanner.scan()) {
      if (
        ![
          ts.SyntaxKind.WhitespaceTrivia,
          ts.SyntaxKind.SingleLineCommentTrivia,
          ts.SyntaxKind.MultiLineCommentTrivia,
          ts.SyntaxKind.NewLineTrivia,
        ].includes(token)
      )
        break;
      at = scanner.getTextPos();
      if (token === ts.SyntaxKind.NewLineTrivia) break;
    }
  }
  return at;
}

/** Reuse only a visible runtime binding; type-only imports and shadowed names do not qualify. */
export function patternImport(file: ts.SourceFile, checker: ts.TypeChecker, location: ts.Node): PatternImport {
  return authoringImport(file, checker, location, "Pattern");
}

export function authoringImport(
  file: ts.SourceFile,
  checker: ts.TypeChecker,
  location: ts.Node,
  exportedName: "Pattern" | "pluginConfig" | "orderEffects",
): PatternImport {
  const f = ts.factory;
  const imports = file.statements.filter(ts.isImportDeclaration);
  const sdkImports = imports.filter(
    (statement) =>
      ts.isStringLiteral(statement.moduleSpecifier) &&
      ["oxitone", "@oxitone/core"].includes(statement.moduleSpecifier.text),
  );
  const visible = new Set(checker.getSymbolsInScope(location, ts.SymbolFlags.Value | ts.SymbolFlags.Alias));
  for (const statement of sdkImports) {
    const clause = statement.importClause;
    if (!clause || clause.isTypeOnly) continue;
    const bindings = clause.namedBindings;
    if (!bindings) continue;
    if (ts.isNamespaceImport(bindings)) {
      const symbol = checker.getSymbolAtLocation(bindings.name);
      if (symbol && visible.has(symbol))
        return { expression: f.createPropertyAccessExpression(f.createIdentifier(bindings.name.text), exportedName) };
    } else
      for (const binding of bindings.elements) {
        const symbol = checker.getSymbolAtLocation(binding.name);
        if (
          !binding.isTypeOnly &&
          (binding.propertyName ?? binding.name).text === exportedName &&
          symbol &&
          visible.has(symbol)
        ) {
          return { expression: f.createIdentifier(binding.name.text) };
        }
      }
  }
  if (file.statements.some(ts.isImportEqualsDeclaration))
    throw new OxitoneError(ErrorCode.EditNotRepresentable, "CommonJS import assignment requires a module adapter");
  // Avoid names anywhere in the file, including deeper scopes and unresolved global references.
  const names = new Set<string>();
  const visit = (node: ts.Node) => {
    if (ts.isIdentifier(node)) names.add(node.text);
    ts.forEachChild(node, visit);
  };
  visit(file);
  let name: string = exportedName;
  for (let suffix = 1; names.has(name); suffix++)
    name = `Local${exportedName[0]!.toUpperCase()}${exportedName.slice(1)}${suffix}`;
  const module = sdkImports[0]?.moduleSpecifier;
  const from = module && ts.isStringLiteral(module) ? module.text : installedAuthoringModule(file.fileName);
  const newline = file.text.includes("\r\n") ? "\r\n" : "\n";
  const at = importPosition(file);
  const prefix = at > 0 && !/[\r\n]/.test(file.text[at - 1] ?? "") ? newline : "";
  const statement = formatSourceStatement(
    file.fileName,
    file.text,
    `import { ${exportedName}${name === exportedName ? "" : ` as ${name}`} } from ${JSON.stringify(from)};`,
  );
  return {
    expression: f.createIdentifier(name),
    patch: { at, text: `${prefix}${statement}${newline}` },
  };
}

/** Helper-only project modules may have no SDK import and only @oxitone/core installed. */
function installedAuthoringModule(fileName: string): string {
  const options = { moduleResolution: ts.ModuleResolutionKind.Bundler, module: ts.ModuleKind.ESNext, allowJs: true };
  return (
    ["oxitone", "@oxitone/core"].find((name) => ts.resolveModuleName(name, fileName, options, ts.sys).resolvedModule) ??
    "oxitone"
  );
}
