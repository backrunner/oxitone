import {
  ErrorCode,
  OxitoneError,
  type TimeSignatureSegment,
} from "@oxitone/protocol";

/** Bar/beat authoring position; bars count from 1, beats from 0. */
export interface BarBeatPosition {
  bar: number;
  beat?: number;
}

function isPowerOfTwo(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && (value & (value - 1)) === 0;
}

function beatsPerBar(segment: TimeSignatureSegment): number {
  return (segment.numerator * 4) / segment.denominator;
}

function validateSignature(numerator: number, denominator: number): void {
  if (!Number.isInteger(numerator) || numerator < 1) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `time signature numerator must be an integer >= 1, got ${numerator}`,
      { details: { path: "timeSignatureMap.numerator" } },
    );
  }
  if (!isPowerOfTwo(denominator)) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `time signature denominator must be a power of two, got ${denominator}`,
      { details: { path: "timeSignatureMap.denominator" } },
    );
  }
}

/**
 * Ordered time-signature map. Changes are declared per bar (the only legal
 * boundary), bars count from 1, and the map always starts with a segment at
 * bar 1.
 */
export class TimeSignatureMap {
  private segments: TimeSignatureSegment[] = [
    { startBar: 1, numerator: 4, denominator: 4 },
  ];

  /** Replace the whole map with a single signature starting at bar 1. */
  set(numerator: number, denominator: number): void {
    validateSignature(numerator, denominator);
    this.segments = [{ startBar: 1, numerator, denominator }];
  }

  /** Append a signature change; `startBar` must exceed every existing one. */
  add(segment: TimeSignatureSegment): void {
    validateSignature(segment.numerator, segment.denominator);
    if (!Number.isInteger(segment.startBar) || segment.startBar < 1) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `time signature startBar must be an integer >= 1, got ${segment.startBar}`,
        { details: { path: "timeSignatureMap.startBar" } },
      );
    }
    const last = this.segments[this.segments.length - 1];
    if (last !== undefined && segment.startBar <= last.startBar) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `time signature startBar ${segment.startBar} must be greater than ${last.startBar}`,
        { details: { path: "timeSignatureMap.startBar" } },
      );
    }
    this.segments.push({ ...segment });
  }

  /** The signature in effect at the given bar. */
  atBar(bar: number): TimeSignatureSegment {
    let current = this.segments[0];
    for (const segment of this.segments) {
      if (segment.startBar > bar) {
        break;
      }
      current = segment;
    }
    if (current === undefined) {
      throw new OxitoneError(ErrorCode.InvalidProject, "time signature map is empty");
    }
    return { ...current };
  }

  /** Beats in one bar at `bar`, used by fractional fit helpers. */
  beatsPerBarAt(bar: number): number {
    return beatsPerBar(this.atBar(bar));
  }

  /** Defensive copy of the segments, as stored on the wire. */
  list(): TimeSignatureSegment[] {
    return this.segments.map((segment) => ({ ...segment }));
  }

  /** Inverse for authoring/display helpers; serialized rational positions remain unchanged. */
  fromBeats(absoluteBeat: number): BarBeatPosition {
    if (!Number.isFinite(absoluteBeat) || absoluteBeat < 0) {
      throw new OxitoneError(ErrorCode.InvalidProject, "absolute beat must be finite and nonnegative");
    }
    let remaining = absoluteBeat;
    for (let i = 0; i < this.segments.length; i++) {
      const segment = this.segments[i]!;
      const next = this.segments[i + 1];
      const perBar = beatsPerBar(segment);
      const span = next === undefined ? Infinity : (next.startBar - segment.startBar) * perBar;
      if (remaining < span) {
        const bars = Math.floor(remaining / perBar);
        return { bar: segment.startBar + bars, beat: remaining - bars * perBar };
      }
      remaining -= span;
    }
    throw new OxitoneError(ErrorCode.InvalidProject, "time signature map is empty");
  }

  /**
   * Convert a bar/beat position to absolute project beats, accumulating
   * across signature changes. `beat` must fit inside the bar it names.
   */
  toBeats(position: BarBeatPosition): number {
    const { bar } = position;
    const beat = position.beat ?? 0;
    if (!Number.isInteger(bar) || bar < 1) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `bar must be an integer >= 1, got ${bar}`,
      );
    }
    if (!Number.isFinite(beat) || beat < 0) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `beat must be finite and >= 0, got ${beat}`,
      );
    }
    let beats = 0;
    for (let i = 0; i < this.segments.length; i += 1) {
      const segment = this.segments[i];
      const next = this.segments[i + 1];
      if (segment === undefined) {
        break;
      }
      const perBar = beatsPerBar(segment);
      if (next === undefined || bar < next.startBar) {
        if (beat >= perBar) {
          throw new OxitoneError(
            ErrorCode.InvalidProject,
            `beat ${beat} is out of range for ${segment.numerator}/${segment.denominator}`,
          );
        }
        return beats + (bar - segment.startBar) * perBar + beat;
      }
      beats += (next.startBar - segment.startBar) * perBar;
    }
    throw new OxitoneError(ErrorCode.InvalidProject, "time signature map is empty");
  }
}
