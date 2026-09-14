import { ErrorCode, OxitoneError, PATTERN_SOURCE_LIMITS, Pcg32, type PatternSourceNode } from "@oxitone/protocol";
import type { SourceEvent, SourceOutput, SourceValue } from "./types.js";

const INTERVALS = {
  major: [0, 4, 7],
  minor: [0, 3, 7],
  dim: [0, 3, 6],
  aug: [0, 4, 8],
  sus2: [0, 2, 7],
  sus4: [0, 5, 7],
} as const;

export function checkEventBudget(count: number): void {
  if (!Number.isSafeInteger(count) || count > PATTERN_SOURCE_LIMITS.events) {
    throw new OxitoneError(ErrorCode.BudgetExceeded, "pattern source exceeds event budget", {
      details: { path: "pattern.source", limit: PATTERN_SOURCE_LIMITS.events },
    });
  }
}

export function resolveChord(node: Extract<PatternSourceNode, { kind: "chord" }>): SourceOutput {
  const { options, root, quality } = node;
  const inversion = options.inversion ?? 0;
  // Equivalent to rotating inversion times, without unbounded control-thread work.
  const turns = Math.floor(inversion / 3);
  const remainder = inversion % 3;
  const pitches = INTERVALS[quality].map((interval, degree) => ({
    pitch: root + interval + 12 * (turns + (degree < remainder ? 1 : 0)),
    degree: degree + 1,
  }));
  pitches.push(...pitches.splice(0, remainder));
  if (options.voicing === "open") {
    const second = pitches[1];
    if (second) second.pitch -= 12;
    pitches.sort((a, b) => a.pitch - b.pitch);
  }
  const start = options.start ?? 0;
  const duration = options.duration ?? 1;
  return {
    lengthBeats: options.lengthBeats ?? start + duration,
    events: pitches.map(({ pitch, degree }, voice) => ({
      note: { pitch: Math.min(pitch, 127), start, duration, velocity: options.velocity ?? 1, voice },
      select: { degree },
      origin: { kind: "chord", degree, voice },
    })),
  };
}

export function resolveArp(node: Extract<PatternSourceNode, { kind: "arp" }>, input: SourceValue): SourceOutput {
  if (input.events.length === 0) throw new OxitoneError(ErrorCode.InvalidProject, "arp requires at least one note");
  const { order, options, rate } = node;
  const cycleLength =
    order === "upDown" ? input.events.length + Math.max(0, input.events.length - 2) : input.events.length;
  checkEventBudget(cycleLength * (options.octaves ?? 1));
  const cycle = input.events.map((event, inputOccurrence) => ({ event, inputOccurrence }));
  if (order === "random") {
    const seed = BigInt(options.seed ?? "0");
    if (seed > 0xffff_ffff_ffff_ffffn) throw new OxitoneError(ErrorCode.InvalidProject, "arp seed must fit u64");
    const rng = new Pcg32(seed);
    for (let i = cycle.length - 1; i > 0; i--) {
      const j = Math.floor(rng.nextFloat() * (i + 1));
      const a = cycle[i];
      const b = cycle[j];
      if (a && b) {
        cycle[i] = b;
        cycle[j] = a;
      }
    }
  } else {
    cycle.sort((a, b) => (order === "down" ? -1 : 1) * (a.event.note.pitch - b.event.note.pitch));
    if (order === "upDown") cycle.push(...cycle.slice(1, -1).reverse());
  }
  const octaves = options.octaves ?? 1;
  const count = cycle.length * octaves;
  checkEventBudget(count);
  const events: SourceEvent[] = [];
  for (let octave = 0; octave < octaves; octave++) {
    for (const [cycleStep, { event, inputOccurrence }] of cycle.entries()) {
      const step = events.length;
      const t = count > 1 ? step / (count - 1) : 0;
      const curve = options.velocityCurve;
      events.push({
        note: {
          pitch: Math.min(event.note.pitch + octave * 12, 127),
          start: step * rate,
          duration: rate * (options.gate ?? 0.9),
          velocity: (options.velocity ?? 1) * (curve ? curve.from + (curve.to - curve.from) * t : 1),
          voice: step,
        },
        select: { step },
        origin: { kind: "arp", step, input: event.origin, inputOccurrence, cycleStep, octave },
      });
    }
  }
  return { events, lengthBeats: options.lengthBeats ?? count * rate };
}
