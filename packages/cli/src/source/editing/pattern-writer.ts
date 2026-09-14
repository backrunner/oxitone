import ts from "typescript";
import { Pattern } from "@oxitone/core";
import { canonicalEncode, ErrorCode, noteEditSchema, OxitoneError, type NoteEdit, type PatternSourceDocument } from "@oxitone/protocol";
import { hasComments, printOperations, readLiteral } from "../syntax/literals.js";
import { expressionAt, sourceHash as hash, sourceProgram } from "../syntax/program.js";

/** Revision-local source anchor. Never printed into the user's TypeScript. */
export interface ExpressionAnchor {
  readonly sourceHash: string;
  readonly start: number;
  readonly end: number;
  readonly expression: string;
}
export interface PatternWriteRequest {
  fileName: string;
  text: string;
  anchor: ExpressionAnchor;
  /** Accepted evaluation of exactly this expression, supplied by the document owner. */
  source: PatternSourceDocument;
  operations: readonly NoteEdit[];
  lengthBeats?: number;
}
export interface PatternWriteResult {
  text: string;
  anchor: ExpressionAnchor;
  source: PatternSourceDocument;
}

function parse(fileName: string, text: string): ts.SourceFile {
  return sourceProgram(fileName, text).file;
}

export function anchorPatternExpression(fileName: string, text: string, start: number, end: number): ExpressionAnchor {
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end <= start || end > text.length) {
    throw new OxitoneError(ErrorCode.EditNotRepresentable, "invalid source range");
  }
  expressionAt(parse(fileName, text), start, end);
  return { sourceHash: hash(text), start, end, expression: text.slice(start, end) };
}

/** Pure minimal patch primitive. The owner must evaluate/validate the full candidate before accepting it. */
export function writePatternEdit(request: PatternWriteRequest): PatternWriteResult {
  const { text, anchor, fileName } = request;
  if (hash(text) !== anchor.sourceHash || text.slice(anchor.start, anchor.end) !== anchor.expression) {
    throw new OxitoneError(ErrorCode.SourceChanged, "source changed since the expression was evaluated", { details: { path: fileName } });
  }
  const file = parse(fileName, text);
  const expression = expressionAt(file, anchor.start, anchor.end);
  const base = Pattern.fromSource(request.source);
  const edited = base.edit(request.operations, request.lengthBeats === undefined ? {} : { lengthBeats: request.lengthBeats });
  if (edited === base) return { text, anchor, source: base.toSource() };
  const source = edited.toSource();
  const root = source.nodes[source.root];
  const oldRoot = request.source.nodes[request.source.root];
  let start = anchor.start;
  let end = anchor.end;
  let replacement = `(${anchor.expression}).edit(${printOperations(request.operations, file)}${request.lengthBeats === undefined ? "" : `, { lengthBeats: ${request.lengthBeats} }`})`;
  // Only replace a literal list whose evaluated rules match the accepted outer edit.
  // Preserve commented/computed lists by wrapping their output instead.
  if (oldRoot?.kind === "edit" && root?.kind === "edit" && source.nodes.length === request.source.nodes.length &&
    ts.isCallExpression(expression) && ts.isPropertyAccessExpression(expression.expression) && expression.expression.name.text === "edit" &&
    expression.arguments.length >= 1 && expression.arguments.length <= 2) {
    const argument = expression.arguments[0];
    const options = expression.arguments[1];
    let sameOptions = !options && oldRoot.lengthBeats === undefined;
    if (options && !hasComments(options.getFullText(file))) {
      try { sameOptions = canonicalEncode(readLiteral(options)) === canonicalEncode({ lengthBeats: oldRoot.lengthBeats }); }
      catch { /* Computed options remain executable TS. */ }
    }
    if (argument && sameOptions && !hasComments(argument.getFullText(file))) {
      let prior: NoteEdit[] | undefined;
      try {
        const literal = readLiteral(argument);
        prior = Array.isArray(literal) ? literal.map((op) => noteEditSchema.parse(op)) : undefined;
      } catch { /* A computed argument remains ordinary executable TS. */ }
      const reusable = prior !== undefined && (canonicalEncode(prior) === canonicalEncode(oldRoot.operations) ||
        (prior.every((op) => "set" in op && op.expect === undefined) &&
          canonicalEncode(base.edit(prior).toSource()) === canonicalEncode(base.toSource())));
      if (reusable) {
        start = argument.getStart(file); end = options?.end ?? argument.end;
        replacement = printOperations(root.operations, file) + (root.lengthBeats === undefined ? "" : `, { lengthBeats: ${root.lengthBeats} }`);
      }
    }
  }
  const indent = text.slice(text.lastIndexOf("\n", start - 1) + 1, start).match(/^[\t ]*/)?.[0] ?? "";
  const newline = text.includes("\r\n") ? "\r\n" : "\n";
  replacement = replacement.replace(/\n/g, `${newline}${indent}`);
  const candidate = text.slice(0, start) + replacement + text.slice(end);
  const nextEnd = anchor.end + replacement.length - (end - start);
  return { text: candidate, source, anchor: anchorPatternExpression(fileName, candidate, anchor.start, nextEnd) };
}
