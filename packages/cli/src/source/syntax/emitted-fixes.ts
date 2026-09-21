import { diffChars } from "diff";
import type { ESLint } from "eslint";

/** Only added/replaced bytes belong to the writer; shared source stays untouched. */
export function applyEmittedFixes(before: string, candidate: string, messages: ESLint.LintResult["messages"]): string {
  const changes = diffChars(before, candidate, { timeout: 50, maxEditLength: 4096 });
  if (!changes) return candidate;
  const emitted: Array<[number, number]> = [];
  let position = 0;
  for (const change of changes) {
    if (change.removed) continue;
    const end = position + change.value.length;
    if (change.added) emitted.push([position, end]);
    position = end;
  }
  // ESLint parses without the BOM and reports fixes in that coordinate space.
  const offset = candidate.startsWith("\uFEFF") ? 1 : 0;
  const fixes = messages
    .flatMap((message) => (message.fix ? [message.fix] : []))
    .filter(({ range: [start, end] }) => start >= 0 && end >= start)
    .map(({ range: [start, end], text }) => ({ range: [start + offset, end + offset] as const, text }))
    .filter(({ range: [start, end] }) => emitted.some(([from, to]) => start >= from && end <= to))
    .sort((a, b) => a.range[0] - b.range[0] || a.range[1] - b.range[1]);
  let result = "",
    cursor = 0,
    lastEnd = -1;
  for (const {
    range: [start, end],
    text,
  } of fixes) {
    // Like ESLint, defer overlapping and same-offset fixes to a later pass.
    if (start <= lastEnd) continue;
    result += candidate.slice(cursor, start) + text;
    cursor = lastEnd = end;
  }
  return result + candidate.slice(cursor);
}
