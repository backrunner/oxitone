import type { NoteSelector } from "@oxitone/protocol";
import type { NoteOrigin, SourceEvent } from "./types.js";

/** Canonical musical selector key; never serialized as entity identity. */
export function selectorKey(select: NoteSelector): string {
  if ("step" in select) return `step:${select.step}`;
  if ("degree" in select) return `degree:${select.degree}`;
  if ("voice" in select) return `voice:${select.voice}`;
  if ("inserted" in select) return `inserted:${select.inserted}`;
  if ("segment" in select) return `segment:${select.segment}/${selectorKey(select.note)}`;
  if ("iteration" in select) return `iteration:${select.iteration}/${selectorKey(select.note)}`;
  return `at:${select.at.start}:${select.at.pitch}:${select.at.voice ?? "*"}:${select.occurrence ?? "*"}`;
}

/** Different shapes may alias the same output (degree/voice or optional literal conditions). */
export function selectorShape(select: NoteSelector): string {
  if ("segment" in select) return `segment/${selectorShape(select.note)}`;
  if ("iteration" in select) return `iteration/${selectorShape(select.note)}`;
  if ("at" in select) return `at/${select.at.voice === undefined}/${select.occurrence === undefined}`;
  return Object.keys(select)[0] ?? "";
}

function outputKeys(select: NoteSelector, origin: NoteOrigin): string[] {
  if ("segment" in select) return outputKeys(select.note, origin).map((key) => `segment:${select.segment}/${key}`);
  if ("iteration" in select)
    return outputKeys(select.note, origin.kind === "repeat" ? origin.input : origin).map(
      (key) => `iteration:${select.iteration}/${key}`,
    );
  if ("at" in select) {
    const prefix = `at:${select.at.start}:${select.at.pitch}:`;
    return [
      ...new Set([
        selectorKey(select),
        `${prefix}*:${select.occurrence ?? "*"}`,
        `${prefix}${select.at.voice ?? "*"}:*`,
        `${prefix}*:*`,
      ]),
    ];
  }
  const keys = [selectorKey(select)];
  if ("degree" in select && origin.kind === "chord") keys.push(`voice:${origin.voice}`);
  return keys;
}

/** Index only requested coordinates; -1 means ambiguous. O(outputs + operations), not O(N*M). */
export function indexSelectors(base: readonly SourceEvent[], selectors: readonly NoteSelector[]): Map<string, number> {
  const requested = new Set(selectors.map(selectorKey));
  const result = new Map<string, number>();
  base.forEach((event, index) => {
    for (const key of outputKeys(event.select, event.origin)) {
      if (requested.has(key)) result.set(key, result.has(key) ? -1 : index);
    }
  });
  return result;
}
