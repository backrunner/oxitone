import ts from "typescript";
import { ErrorCode, OxitoneError, type ProjectSnapshot } from "@oxitone/protocol";
import { authoringImport } from "../syntax/imports.js";
import { hasComments, literal, readLiteral } from "../syntax/literals.js";
import { expressionAt, sourceHash, sourceProgram } from "../syntax/program.js";
import type { EvaluatedEffectOwnerSite, ProjectEvaluation } from "../eval/project-evaluation.js";
import { sourceSpan } from "../eval/source-timing.js";

/** Late fluent Project configuration must be ordered after the final Project expression runs. */
export function projectOrderEdit(before: ProjectEvaluation, owner: string, order: readonly string[]): import("@oxitone/protocol").ProjectEdit | undefined {
  const late = before.configurationSites.some(config => config.usages.some(u => u.owner === owner) && before.projectSites.some(project =>
    project.fileName === config.fileName && project.anchor.start <= config.anchor.start && project.anchor.end >= config.anchor.end));
  if (!late) return undefined;
  const channel = before.frame.snapshot.channels.find(c => c.id === owner);
  const effects = channel?.effectChain ?? before.frame.snapshot.mixerChannels.find(b => b.id === owner)?.inserts;
  const index = (channel ? before.arrangementOrder.channels : before.arrangementOrder.mixerChannels).indexOf(owner);
  if (!effects || index < 0) throw new OxitoneError(ErrorCode.EditTargetMissing, "effect owner no longer exists");
  const permutation = order.map(id => effects.findIndex(effect => effect.instanceId === id));
  if (order.length !== effects.length || new Set(order).size !== order.length || permutation.some(i => i < 0)) throw new OxitoneError(ErrorCode.EditScopeConflict, "order must contain every current instance once");
  return { kind: "effectOrder", owner: channel ? "channel" : "bus", index, order: permutation };
}

/** Order after existing bindings are created. Only chain order may differ in the accepted candidate. */
export function writeEffectOrder(text: string, site: EvaluatedEffectOwnerSite, snapshot: ProjectSnapshot, order: readonly string[]) {
  const done = sourceSpan("effect-order-writing");
  try { return effectOrder(text, site, snapshot, order); } finally { done(); }
}

function effectOrder(text: string, site: EvaluatedEffectOwnerSite, snapshot: ProjectSnapshot, order: readonly string[]) {
  const { anchor, fileName } = site;
  if (sourceHash(text) !== anchor.sourceHash || text.slice(anchor.start, anchor.end) !== anchor.expression) throw new OxitoneError(ErrorCode.SourceChanged, "effect owner source changed");
  const expected = structuredClone(snapshot);
  const owner = expected.channels.find(channel => channel.id === site.owner) ?? expected.mixerChannels.find(bus => bus.id === site.owner);
  if (!owner) throw new OxitoneError(ErrorCode.EditTargetMissing, "effect owner no longer exists");
  const effects = "instrument" in owner ? owner.effectChain : owner.inserts;
  const permutation = order.map(id => effects.findIndex(effect => effect.instanceId === id));
  if (order.length !== effects.length || new Set(order).size !== order.length || permutation.some(index => index < 0)) throw new OxitoneError(ErrorCode.EditScopeConflict, "order must contain every current instance of this owner once");
  const reordered = permutation.map(index => effects[index]!);
  if ("instrument" in owner) owner.effectChain = reordered; else owner.inserts = reordered;
  if (permutation.every((index, position) => index === position)) return { text, expected };
  const { file, checker } = sourceProgram(fileName, text);
  const expression = expressionAt(file, anchor.start, anchor.end);
  const declaration = expression.parent;
  if (!ts.isVariableDeclaration(declaration) || declaration.initializer !== expression || !ts.isIdentifier(declaration.name)
    || !ts.isVariableDeclarationList(declaration.parent) || !ts.isVariableStatement(declaration.parent.parent)) {
    throw new OxitoneError(ErrorCode.EditNotRepresentable, "effect reordering requires a local owner variable in a module or block");
  }
  const block = declaration.parent.parent.parent;
  if (!ts.isSourceFile(block) && !ts.isBlock(block)) throw new OxitoneError(ErrorCode.EditNotRepresentable, "owner scope cannot hold a local ordering statement");
  const last = block.statements.at(-1), terminal = last && ts.isReturnStatement(last) ? last : undefined;
  const previous = terminal ? block.statements.at(-2) : last;
  const at = terminal ? terminal.getFullStart() : ts.isSourceFile(block) ? file.endOfFileToken.getFullStart() : block.end - 1;
  const imported = authoringImport(file, checker, terminal ?? block, "orderEffects");
  const printer = ts.createPrinter({ newLine: ts.NewLineKind.LineFeed });
  const print = (node: ts.Expression) => printer.printNode(ts.EmitHint.Expression, node, file);
  let indices = permutation, start = at, end = at;
  if (previous && ts.isExpressionStatement(previous) && ts.isCallExpression(previous.expression) && !hasComments(previous.getText(file))) {
    const call = previous.expression, target = call.arguments[0];
    if (call.arguments.length === 2 && target && ts.isIdentifier(target) && checker.getSymbolAtLocation(target) === checker.getSymbolAtLocation(declaration.name) && print(call.expression) === print(imported.expression)) {
      try {
        const prior = readLiteral(call.arguments[1]!);
        if (Array.isArray(prior) && prior.length === indices.length && new Set(prior).size === prior.length && prior.every(index => Number.isInteger(index) && index >= 0 && index < prior.length)) {
          indices = permutation.map(index => prior[index] as number); start = previous.getStart(file); end = previous.end;
        }
      } catch { /* Nonliteral calls retain their normal evaluation. */ }
    }
  }
  const identity = indices.every((index, position) => index === position);
  const newline = text.includes("\r\n") ? "\r\n" : "\n";
  const indent = text.slice(text.lastIndexOf("\n", declaration.getStart(file)) + 1, declaration.parent.parent.getStart(file)).match(/^\s*/)?.[0] ?? "";
  const statement = identity ? "" : print(ts.factory.createCallExpression(imported.expression, undefined, [ts.factory.createIdentifier(declaration.name.text), literal(indices)])) + ";";
  let result = text.slice(0, start) + (start === end ? newline + indent + statement + newline : statement) + text.slice(end);
  if (!identity && imported.patch) result = result.slice(0, imported.patch.at) + imported.patch.text + result.slice(imported.patch.at);
  return { text: result, expected };
}
