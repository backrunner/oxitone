import { note, type Hit } from "./shared.js";

/** Four-bar cells develop within each eight-bar statement, without rewriting the lead. */
export function dropPhrase(bar: number) {
  const second = bar >= 72, position = bar - (second ? 72 : 24);
  const open = second && position >= 8, turn = position % 8 === 7;
  const chords = open ? [0, 1.5, 3] : position % 2 ? [0, 0.75, 1.5, 2.5, 3.25] : [0, 1.5, 2.5, 3.5];
  // The eighth-bar turnaround hands the last beat to bass and delay tails.
  const accents = turn ? chords.filter(t => t < 3) : chords;
  const kicks = position % 4 === 1 ? [0, 0.75, 3.25] : position % 4 === 2 ? [0, 1.25, 3.5] : [0, 1.5];
  return { second, position, open, turn, accents, kicks };
}

export function bassAnswer(root: number, bar: number): Hit[] {
  const { position, second } = dropPhrase(bar);
  // FM and vowel answers alternate: they never compete at the same onset.
  const times = position % 2 ? [0.5, 1.25] : [0.75];
  return times.map((t, i) => note(root + 12 + (second && i === 1 ? 7 : 0), t, i ? 0.23 : 0.42, 0.88 - i * 0.06));
}

export function dropHats(bar: number): Hit[] {
  const { position, second, turn } = dropPhrase(bar);
  const grid = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5];
  const hits = grid.filter(t => !turn || t < 3).map(t => note(t === 3.5 && position % 2 ? 46 : 42,
    t + (t % 1 ? 0.018 : 0), 0.05, t % 1 ? second ? 0.68 : 0.61 : 0.42));
  if (position % 4 === 2) hits.push(note(42, 3.25, 0.05, 0.3), note(42, 3.75, 0.05, 0.36));
  return hits;
}
