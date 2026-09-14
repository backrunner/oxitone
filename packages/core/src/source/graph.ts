import {
  ErrorCode,
  OxitoneError,
  PATTERN_SOURCE_FORMAT,
  PATTERN_SOURCE_LIMITS,
  patternSourceDocumentSchema,
  patternSourceNodeSchema,
  type PatternSourceDocument,
  type PatternSourceNode,
} from "@oxitone/protocol";
import { parseAuthoring } from "../authoring-validation.js";
import { validateNote } from "../notes/note.js";
import { sourceNoteInput } from "./notes.js";
import { resolveSource, sourceReferences, withReferences } from "./evaluate.js";
import { freezeSource, type SourceValue } from "./types.js";

/** Reject recursive/cyclic selectors before entering the recursive schema parser. */
function checkSelectors(node: unknown): void {
  if (typeof node !== "object" || node === null || !("operations" in node) || !Array.isArray(node.operations)) return;
  for (const operation of node.operations as unknown[]) {
    if (typeof operation !== "object" || operation === null || !("select" in operation)) continue;
    let select: unknown = operation.select;
    let depth = 0;
    while (typeof select === "object" && select !== null && "note" in select) {
      if (++depth > PATTERN_SOURCE_LIMITS.depth)
        throw new OxitoneError(ErrorCode.BudgetExceeded, "note selector exceeds depth budget");
      select = select.note;
    }
  }
}

export function validateSourceNode(node: PatternSourceNode): PatternSourceNode {
  checkSelectors(node);
  return parseAuthoring(patternSourceNodeSchema, node, "pattern.source");
}

export function createSource(node: PatternSourceNode, inputs: readonly SourceValue[] = []): SourceValue {
  const depth = 1 + Math.max(0, ...inputs.map((input) => input.depth));
  if (depth > PATTERN_SOURCE_LIMITS.depth)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "pattern source exceeds depth budget");
  const parsed = validateSourceNode(node);
  if (parsed.kind === "arp" && BigInt(parsed.options.seed ?? "0") > 0xffff_ffff_ffff_ffffn) {
    throw new OxitoneError(ErrorCode.InvalidProject, "arp seed must fit u64");
  }
  const output = resolveSource(parsed, inputs);
  if (!Number.isFinite(output.lengthBeats) || output.lengthBeats <= 0)
    throw new OxitoneError(ErrorCode.InvalidProject, "source length must be finite and positive");
  const events = output.events
    .map((event) => {
      validateNote(sourceNoteInput(event.note));
      return event;
    })
    .sort((a, b) => a.note.start - b.note.start || (a.note.voice ?? 0) - (b.note.voice ?? 0));
  return freezeSource({ node: parsed, inputs: [...inputs], events, lengthBeats: output.lengthBeats, depth });
}

export function serializeSource(value: SourceValue, name?: string): PatternSourceDocument {
  const nodes: PatternSourceNode[] = [];
  const indexes = new Map<SourceValue, number>();
  let totalEvents = 0;
  function visit(source: SourceValue): number {
    const existing = indexes.get(source);
    if (existing !== undefined) return existing;
    const refs = source.inputs.map(visit);
    const index = nodes.length;
    if (index >= PATTERN_SOURCE_LIMITS.nodes)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "pattern source exceeds node budget");
    totalEvents += source.events.length;
    if (totalEvents > PATTERN_SOURCE_LIMITS.events * 8)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "pattern graph exceeds total event budget");
    nodes.push(withReferences(source.node, refs));
    indexes.set(source, index);
    return index;
  }
  const root = visit(value);
  return structuredClone({
    formatVersion: PATTERN_SOURCE_FORMAT,
    nodes,
    root,
    ...(name === undefined ? {} : { name }),
  });
}

export function restoreSource(input: unknown): { source: SourceValue; name?: string } {
  if (
    typeof input === "object" &&
    input !== null &&
    "formatVersion" in input &&
    input.formatVersion !== PATTERN_SOURCE_FORMAT
  ) {
    throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "unsupported pattern source format");
  }
  if (typeof input === "object" && input !== null && "nodes" in input && Array.isArray(input.nodes)) {
    if (input.nodes.length > PATTERN_SOURCE_LIMITS.nodes)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "pattern source exceeds node budget");
    for (const node of input.nodes) checkSelectors(node);
  }
  const document = parseAuthoring(patternSourceDocumentSchema, input, "pattern.source");
  if (document.root !== document.nodes.length - 1)
    throw new OxitoneError(ErrorCode.InvalidProject, "source root must be the final node");
  const reachable = new Set<number>([document.root]);
  for (let i = document.nodes.length - 1; i >= 0; i--) {
    const node = document.nodes[i];
    if (!node || !reachable.has(i))
      throw new OxitoneError(ErrorCode.InvalidProject, "source contains unreachable nodes");
    for (const ref of sourceReferences(node)) {
      if (ref >= i) throw new OxitoneError(ErrorCode.InvalidProject, "source inputs must precede their parent");
      reachable.add(ref);
    }
  }
  const values: SourceValue[] = [];
  let totalEvents = 0;
  for (const node of document.nodes) {
    const refs = sourceReferences(node);
    const inputs = refs.map((ref) => {
      const value = values[ref];
      if (!value) throw new OxitoneError(ErrorCode.InvalidProject, "source reference missing");
      return value;
    });
    const source = createSource(
      withReferences(
        node,
        inputs.map((_, index) => index),
      ),
      inputs,
    );
    totalEvents += source.events.length;
    if (totalEvents > PATTERN_SOURCE_LIMITS.events * 8)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "pattern graph exceeds total event budget");
    values.push(source);
  }
  const source = values[document.root];
  if (!source) throw new OxitoneError(ErrorCode.InvalidProject, "source root missing");
  return { source, ...(document.name === undefined ? {} : { name: document.name }) };
}
