import { beatToWire, beatFromWire, ErrorCode, OxitoneError, type TempoSegment } from "@oxitone/protocol";

/** Transition curve from one tempo segment to the next. */
export type TempoCurve = "step" | "linear" | "exponential";

/** Authoring input for a single tempo segment. */
export interface TempoSegmentInput {
  startBeat: number;
  bpm: number;
  curve?: TempoCurve;
}

export const DEFAULT_BPM = 120;
export const MIN_BPM = 20;
export const MAX_BPM = 999;

function validateBpm(bpm: number): void {
  if (!Number.isFinite(bpm) || bpm < MIN_BPM || bpm > MAX_BPM) {
    throw new OxitoneError(ErrorCode.TempoRange, `bpm must be a finite number in ${MIN_BPM}..${MAX_BPM}, got ${bpm}`, {
      details: { path: "tempoMap.bpm" },
    });
  }
}

/**
 * Ordered, validated tempo map. The first segment always sits at beat 0 and
 * later segments must have strictly increasing start beats.
 */
export class TempoMap {
  private segments: TempoSegmentInput[] = [{ startBeat: 0, bpm: DEFAULT_BPM }];
  private restoredBeats: TempoSegment["startBeat"][] = [];

  /** Replace the whole map with a single segment at beat 0. */
  set(bpm: number, curve?: TempoCurve): void {
    validateBpm(bpm);
    const segment: TempoSegmentInput = { startBeat: 0, bpm };
    if (curve !== undefined) {
      segment.curve = curve;
    }
    this.segments = [segment];
    this.restoredBeats = [];
  }

  /** Append a segment; `startBeat` must be greater than every existing one. */
  add(segment: TempoSegmentInput): void {
    validateBpm(segment.bpm);
    if (!Number.isFinite(segment.startBeat) || segment.startBeat < 0) {
      throw new OxitoneError(
        ErrorCode.TempoMapOrder,
        `tempo segment startBeat must be finite and >= 0, got ${segment.startBeat}`,
        { details: { path: "tempoMap.startBeat" } },
      );
    }
    const last = this.segments[this.segments.length - 1];
    if (last !== undefined && segment.startBeat <= last.startBeat) {
      throw new OxitoneError(
        ErrorCode.TempoMapOrder,
        `tempo segment startBeat ${segment.startBeat} must be greater than ${last.startBeat}`,
        { details: { path: "tempoMap.startBeat" } },
      );
    }
    const copy: TempoSegmentInput = { startBeat: segment.startBeat, bpm: segment.bpm };
    if (segment.curve !== undefined) {
      copy.curve = segment.curve;
    }
    this.segments.push(copy);
  }

  /** Defensive copy of the current segments. */
  list(): TempoSegmentInput[] {
    return this.segments.map((segment) => ({ ...segment }));
  }

  /** @internal Validate the map while preserving exact restored rational positions. */
  restore(segments: TempoSegment[]): void {
    const first = segments[0];
    if (first === undefined || first.startBeat.numerator !== 0) {
      throw new OxitoneError(ErrorCode.TempoMapOrder, "tempo map must start at beat zero");
    }
    this.set(first.bpm, first.curve);
    for (const segment of segments.slice(1))
      this.add({
        startBeat: beatFromWire(segment.startBeat),
        bpm: segment.bpm,
        ...(segment.curve === undefined ? {} : { curve: segment.curve }),
      });
    this.restoredBeats = segments.map((segment) => ({ ...segment.startBeat }));
  }

  /** Wire-form segments with canonicalized beats. */
  toWire(): TempoSegment[] {
    return this.segments.map((segment, index) => {
      const wire: TempoSegment = {
        startBeat: { ...(this.restoredBeats[index] ?? beatToWire(segment.startBeat)) },
        bpm: segment.bpm,
      };
      if (segment.curve !== undefined) {
        wire.curve = segment.curve;
      }
      return wire;
    });
  }
}
