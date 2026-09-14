/** Conservative line based three-way merge. Overlapping edits return undefined. */
export function mergeThreeWay(base: string, draft: string, disk: string): string | undefined {
  if (draft === base) return disk;
  if (disk === base || draft === disk) return draft;
  const baseLines = lines(base),
    draftLines = lines(draft),
    diskLines = lines(disk);
  const left = hunks(baseLines, draftLines, 0),
    right = hunks(baseLines, diskLines, 1);
  if (!left || !right) return undefined;
  const all = [...left, ...right].sort((a, b) => a.start - b.start || a.end - b.end || a.side - b.side);
  const selected: Hunk[] = [];
  for (const candidate of all) {
    const previous = selected[selected.length - 1];
    if (!previous) {
      selected.push(candidate);
      continue;
    }
    const overlap =
      (candidate.start < previous.end && previous.start < candidate.end) ||
      (candidate.start === candidate.end && previous.start === previous.end && candidate.start === previous.start);
    if (!overlap) {
      selected.push(candidate);
      continue;
    }
    if (
      candidate.start === previous.start &&
      candidate.end === previous.end &&
      sameLines(candidate.replacement, previous.replacement)
    )
      continue;
    return undefined;
  }
  const output: string[] = [];
  let cursor = 0;
  for (const hunk of selected) {
    output.push(...baseLines.slice(cursor, hunk.start), ...hunk.replacement);
    cursor = hunk.end;
  }
  output.push(...baseLines.slice(cursor));
  return output.join("");
}

type Hunk = { start: number; end: number; replacement: string[]; side: 0 | 1 };
const lines = (text: string): string[] => text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
const sameLines = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && a.every((line, index) => line === b[index]);

function hunks(base: readonly string[], variant: readonly string[], side: 0 | 1 = 0): Hunk[] | undefined {
  if (base.length * variant.length > 1_000_000) return undefined;
  const table: number[][] = Array.from({ length: base.length + 1 }, () =>
    new Array<number>(variant.length + 1).fill(0),
  );
  for (let i = base.length - 1; i >= 0; i--)
    for (let j = variant.length - 1; j >= 0; j--)
      table[i]![j] =
        base[i] === variant[j] ? table[i + 1]![j + 1]! + 1 : Math.max(table[i + 1]![j]!, table[i]![j + 1]!);
  const result: Hunk[] = [];
  let i = 0,
    j = 0;
  while (i < base.length || j < variant.length) {
    if (i < base.length && j < variant.length && base[i] === variant[j]) {
      i++;
      j++;
      continue;
    }
    const start = i,
      replacement: string[] = [];
    while (i < base.length || j < variant.length) {
      if (i < base.length && j < variant.length && base[i] === variant[j]) break;
      if (i < base.length && (j === variant.length || table[i + 1]![j]! >= table[i]![j + 1]!)) i++;
      else replacement.push(variant[j++]!);
    }
    result.push({ start, end: i, replacement, side });
  }
  return result;
}
