import { ErrorCode, OxitoneError, type NoteEdit, type SourceNote } from "@oxitone/protocol";
import type { SourceEvent } from "./types.js";
import { checkEventBudget } from "./generators.js";
import { indexSelectors, selectorKey, selectorShape } from "./selectors.js";

function matchesExpected(
  note: SourceEvent["note"],
  expect: { [K in keyof SourceNote]?: SourceNote[K] | undefined },
): boolean {
  return Object.entries(expect).every(([key, value]) => {
    if (value === undefined) return true;
    const actual = note[key as keyof SourceNote];
    return Array.isArray(value) ? JSON.stringify(actual) === JSON.stringify(value) : actual === value;
  });
}

/** Resolve every selector and expectation against the same base before applying anything. */
export function resolveEdits(base: readonly SourceEvent[], operations: readonly NoteEdit[]): SourceEvent[] {
  checkEventBudget(base.length + operations.filter((op) => "insert" in op).length);
  const indexed = indexSelectors(
    base,
    operations.flatMap((op) => ("select" in op ? [op.select] : [])),
  );
  const targets = operations.map((op, operation) => {
    if ("insert" in op) return -1;
    const target = indexed.get(selectorKey(op.select));
    if (target === undefined || target === -1) {
      throw new OxitoneError(
        target === undefined ? ErrorCode.EditTargetMissing : ErrorCode.EditTargetAmbiguous,
        "note selector must resolve to exactly one base output",
        { details: { path: "pattern.edit", operation, select: op.select } },
      );
    }
    const event = base[target];
    if (event && op.expect && !matchesExpected(event.note, op.expect)) {
      throw new OxitoneError(ErrorCode.SourceChanged, "note expectation no longer matches", {
        details: { path: "pattern.edit", operation, select: op.select },
      });
    }
    return target;
  });
  const actions = new Map<number, "remove" | "update">();
  operations.forEach((op, i) => {
    const target = targets[i];
    if (target === undefined || target < 0) return;
    const action = "remove" in op ? "remove" : "update";
    const prior = actions.get(target);
    if (prior !== undefined && prior !== action)
      throw new OxitoneError(ErrorCode.EditScopeConflict, "cannot remove and update the same note in one edit", {
        details: { path: "pattern.edit", operation: i },
      });
    actions.set(target, action);
  });
  const result: (SourceEvent | undefined)[] = [...base];
  let inserted = 0;
  const insertCounts = new Map<string, number>();
  // Keep insertion coordinates unique across consecutive edit layers.
  for (const event of base) {
    if ("inserted" in event.select) inserted = Math.max(inserted, event.select.inserted + 1);
    if (event.origin.kind === "insert") {
      const key = JSON.stringify(event.origin.note);
      insertCounts.set(key, Math.max(insertCounts.get(key) ?? 0, event.origin.occurrence + 1));
    }
  }
  operations.forEach((op, i) => {
    if ("insert" in op) {
      const key = JSON.stringify(op.insert);
      const occurrence = insertCounts.get(key) ?? 0;
      insertCounts.set(key, occurrence + 1);
      result.push({
        note: op.insert,
        select: { inserted: inserted++ },
        origin: { kind: "insert", note: op.insert, occurrence },
      });
      return;
    }
    const target = targets[i];
    if (target === undefined) return;
    if ("remove" in op) {
      result[target] = undefined;
      return;
    }
    const event = result[target];
    if (!event) return;
    const note = { ...event.note };
    if ("set" in op)
      Object.assign(note, Object.fromEntries(Object.entries(op.set).filter(([, value]) => value !== undefined)));
    else
      for (const [key, amount] of Object.entries(op.shift)) {
        const field = key as keyof typeof op.shift;
        if (amount !== undefined) note[field] += amount;
      }
    result[target] = { ...event, note };
  });
  return result.filter((event): event is SourceEvent => event !== undefined);
}

/** Reduce repeated absolute drags without changing expectation or insertion semantics. */
export function mergeSets(previous: readonly NoteEdit[], next: readonly NoteEdit[]): NoteEdit[] | undefined {
  const all = [...previous, ...next];
  if (!all.every((op) => "set" in op && op.expect === undefined)) return undefined;
  if (new Set(all.flatMap((op) => ("select" in op ? [selectorShape(op.select)] : []))).size > 1) return undefined;
  const merged: NoteEdit[] = [];
  const bySelector = new Map<string, NoteEdit>();
  for (const op of all) {
    if (!("set" in op)) continue;
    const key = selectorKey(op.select);
    const prior = bySelector.get(key);
    const defined = Object.fromEntries(Object.entries(op.set).filter(([, value]) => value !== undefined));
    if (prior && "set" in prior) prior.set = { ...prior.set, ...defined };
    else {
      const copy = { select: op.select, set: defined };
      merged.push(copy);
      bySelector.set(key, copy);
    }
  }
  return merged;
}
