import ts from "typescript";
import { Pattern } from "@oxitone/core";
import { canonicalEncode, ErrorCode, OxitoneError, type PatternSourceDocument } from "@oxitone/protocol";
import { literal, hasComments, readLiteral } from "../syntax/literals.js";
import { patternImport } from "../syntax/imports.js";
import {
  anchorPatternExpression,
  type ExpressionAnchor,
  type PatternWriteRequest,
  type PatternWriteResult,
} from "./pattern-writer.js";
import { expressionAt, sourceHash, sourceProgram } from "../syntax/program.js";

export interface MaterializeSummary {
  readonly beforeNotes: number;
  readonly afterNotes: number;
  readonly lengthBeats: number;
  readonly scope: "selected-reference";
  readonly losesGeneratorLink: true;
  readonly resetsMusicalOrigins: true;
  readonly retainsDependencyImports: true;
  readonly retainsOriginalEvaluation: boolean;
}
export interface MaterializeCandidate extends PatternWriteResult {
  readonly summary: MaterializeSummary;
}

function patternOptions(pattern: Pattern) {
  return {
    lengthBeats: pattern.lengthBeats,
    ...(pattern.name === undefined ? {} : { name: pattern.name }),
    notes: pattern.notes,
  };
}

function printPattern(pattern: Pattern, constructor: ts.Expression, file: ts.SourceFile): string {
  const f = ts.factory;
  const properties = Object.entries(patternOptions(pattern)).map(([key, value]) =>
    f.createPropertyAssignment(
      key,
      key === "notes" ? f.createArrayLiteralExpression(pattern.notes.map(literal), true) : literal(value),
    ),
  );
  return ts
    .createPrinter({ newLine: ts.NewLineKind.LineFeed })
    .printNode(
      ts.EmitHint.Expression,
      f.createNewExpression(constructor, undefined, [f.createObjectLiteralExpression(properties, true)]),
      file,
    );
}

/** A candidate only. Project ownership, accepted evaluation and user review belong to the document owner. */
export function materializePatternReference(request: PatternWriteRequest): MaterializeCandidate {
  const { text, fileName, anchor } = request;
  if (sourceHash(text) !== anchor.sourceHash || text.slice(anchor.start, anchor.end) !== anchor.expression) {
    throw new OxitoneError(ErrorCode.SourceChanged, "source changed since the reference was evaluated");
  }
  const { file, checker } = sourceProgram(fileName, text);
  const expression = expressionAt(file, anchor.start, anchor.end);
  // A simple bound read is replaceable. Other complete expressions still run exactly once;
  // discard their returned value only, preserving calls, getters, await and thrown errors.
  const symbol = ts.isIdentifier(expression) ? checker.getSymbolAtLocation(expression) : undefined;
  const bound = symbol?.declarations?.some(
    (declaration) =>
      ts.isVariableDeclaration(declaration) ||
      ts.isImportSpecifier(declaration) ||
      ts.isImportClause(declaration) ||
      ts.isParameter(declaration),
  );
  const base = Pattern.fromSource(request.source);
  const edited = base.edit(
    request.operations,
    request.lengthBeats === undefined ? {} : { lengthBeats: request.lengthBeats },
  );
  if ([...base.notes, ...edited.notes].some((note) => note.chance !== undefined && note.chance !== 1)) {
    throw new OxitoneError(
      ErrorCode.EditNotRepresentable,
      "probabilistic notes require the new runtime origin contract before materialization",
    );
  }
  if (request.source.nodes.some((node) => node.kind === "slice"))
    throw new OxitoneError(
      ErrorCode.EditNotRepresentable,
      "source windows require phase-preserving runtime materialization",
    );
  const detached = new Pattern({
    lengthBeats: edited.lengthBeats,
    notes: edited.notes,
    ...(edited.name === undefined ? {} : { name: edited.name }),
  });
  const imported = patternImport(file, checker, expression);
  const literalText = printPattern(detached, imported.expression, file);
  const printed = bound ? literalText : `((${anchor.expression}), ${literalText})`;
  const candidate = replaceExpression(fileName, text, anchor, printed, detached.toSource());
  let result: PatternWriteResult = candidate;
  if (imported.patch) {
    const { at, text: addition } = imported.patch;
    if (at > anchor.start)
      throw new OxitoneError(
        ErrorCode.EditNotRepresentable,
        "cannot insert a runtime import after the selected reference",
      );
    const next = candidate.text.slice(0, at) + addition + candidate.text.slice(at);
    result = {
      text: next,
      source: candidate.source,
      anchor: anchorPatternExpression(
        fileName,
        next,
        candidate.anchor.start + addition.length,
        candidate.anchor.end + addition.length,
      ),
    };
  }
  return {
    ...result,
    summary: {
      beforeNotes: base.notes.length,
      afterNotes: detached.notes.length,
      lengthBeats: detached.lengthBeats,
      scope: "selected-reference",
      losesGeneratorLink: true,
      resetsMusicalOrigins: true,
      retainsDependencyImports: true,
      retainsOriginalEvaluation: !bound,
    },
  };
}

function replaceExpression(
  fileName: string,
  text: string,
  anchor: ExpressionAnchor,
  expression: string,
  source: PatternSourceDocument,
): PatternWriteResult {
  const indent = text.slice(text.lastIndexOf("\n", anchor.start - 1) + 1, anchor.start).match(/^[\t ]*/)?.[0] ?? "";
  const replacement = expression.replace(/\n/g, `${text.includes("\r\n") ? "\r\n" : "\n"}${indent}`);
  const candidate = text.slice(0, anchor.start) + replacement + text.slice(anchor.end);
  return {
    text: candidate,
    source,
    anchor: anchorPatternExpression(fileName, candidate, anchor.start, anchor.start + replacement.length),
  };
}

/** Edit a previously materialized literal directly. Unknown/computed options remain on the normal edit path. */
export function writeLiteralPatternEdit(request: PatternWriteRequest): PatternWriteResult | undefined {
  const { file, checker } = sourceProgram(request.fileName, request.text);
  if (sourceHash(request.text) !== request.anchor.sourceHash)
    throw new OxitoneError(ErrorCode.SourceChanged, "source changed since evaluation");
  const boundary = expressionAt(file, request.anchor.start, request.anchor.end);
  let expression = boundary;
  while (
    ts.isParenthesizedExpression(expression) ||
    (ts.isBinaryExpression(expression) && expression.operatorToken.kind === ts.SyntaxKind.CommaToken)
  ) {
    expression = ts.isParenthesizedExpression(expression) ? expression.expression : expression.right;
  }
  if (!ts.isNewExpression(expression) || expression.arguments?.length !== 1 || hasComments(boundary.getText(file)))
    return undefined;
  const imported = patternImport(file, checker, expression);
  if (imported.patch) return undefined;
  const printer = ts.createPrinter();
  if (expression.expression.getText(file) !== printer.printNode(ts.EmitHint.Expression, imported.expression, file))
    return undefined;
  const argument = expression.arguments[0];
  if (!argument) return undefined;
  // Literal/source equivalence is checked before rewriting; getters/spreads/factory calls are not read.
  const base = Pattern.fromSource(request.source);
  if (request.source.nodes.length !== 1 || request.source.nodes[0]?.kind !== "literal") return undefined;
  try {
    if (canonicalEncode(readLiteral(argument)) !== canonicalEncode(patternOptions(base))) return undefined;
  } catch {
    return undefined;
  }
  if (request.operations.length === 0 && request.lengthBeats === undefined)
    return { text: request.text, anchor: request.anchor, source: base.toSource() };
  const edited = base.edit(
    request.operations,
    request.lengthBeats === undefined ? {} : { lengthBeats: request.lengthBeats },
  );
  if ([...base.notes, ...edited.notes].some((note) => note.chance !== undefined && note.chance !== 1)) return undefined;
  const detached = new Pattern({
    lengthBeats: edited.lengthBeats,
    notes: edited.notes,
    ...(edited.name === undefined ? {} : { name: edited.name }),
  });
  return replaceExpression(
    request.fileName,
    request.text,
    request.anchor,
    request.text.slice(request.anchor.start, expression.getStart(file)) +
      printPattern(detached, imported.expression, file) +
      request.text.slice(expression.end, request.anchor.end),
    detached.toSource(),
  );
}
