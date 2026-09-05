import { beatToWire, ErrorCode, OxitoneError, type PatternClipSpec } from "@oxitone/protocol";
import type { Pattern } from "./pattern.js";
import type { Project } from "./project.js";
import type { BarBeatPosition } from "./time-signature.js";
import type { Track } from "./track.js";

/**
 * A pattern placed on a track timeline. Created via
 * `track.pattern(pattern).at({ bar, beat })`; placement options chain from
 * there. `loopCount` and `lastBeat` are mutually exclusive.
 */
export class PatternClip {
  readonly id: string;
  readonly pattern: Pattern;
  readonly track: Track;
  readonly startBeat: number;
  private readonly project: Project;
  private loopCountValue?: number;
  private lastBeatValue?: number;
  private transposeValue = 0;
  private velocityScaleValue = 1;
  private probabilityValue = 1;
  private enabledValue = true;

  /** @internal Use `track.pattern(pattern).at(...)` instead. */
  constructor(project: Project, track: Track, pattern: Pattern, id: string, startBeat: number) {
    this.project = project;
    this.track = track;
    this.pattern = pattern;
    this.id = id;
    this.startBeat = startBeat;
  }

  get loopCount(): number | undefined {
    return this.loopCountValue;
  }

  get lastBeat(): number | undefined {
    return this.lastBeatValue;
  }

  get transposeSemitones(): number {
    return this.transposeValue;
  }

  get velocityScaleFactor(): number {
    return this.velocityScaleValue;
  }

  get probabilityFactor(): number {
    return this.probabilityValue;
  }

  get isEnabled(): boolean {
    return this.enabledValue;
  }

  /** Repeat the pattern `count` times from `startBeat`. */
  loop(count: number): this {
    if (this.lastBeatValue !== undefined) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "loopCount and lastBeat are mutually exclusive",
        { details: { path: "patternClip.loopCount" } },
      );
    }
    if (!Number.isInteger(count) || count < 1) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `loop count must be an integer >= 1, got ${count}`,
        { details: { path: "patternClip.loopCount" } },
      );
    }
    this.loopCountValue = count;
    this.project.touch();
    return this;
  }

  /** Exclusive end position (bar/beat) for looping; trims trailing notes. */
  last(position: BarBeatPosition): this {
    if (this.loopCountValue !== undefined) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "loopCount and lastBeat are mutually exclusive",
        { details: { path: "patternClip.lastBeat" } },
      );
    }
    const beat = this.project.barBeatToBeats(position);
    if (beat <= this.startBeat) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `lastBeat ${beat} must be after startBeat ${this.startBeat}`,
        { details: { path: "patternClip.lastBeat" } },
      );
    }
    this.lastBeatValue = beat;
    this.project.touch();
    return this;
  }

  /** Transpose all notes by `semitones` (integer). */
  transpose(semitones: number): this {
    if (!Number.isInteger(semitones)) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `transpose must be an integer, got ${semitones}`,
        { details: { path: "patternClip.transpose" } },
      );
    }
    this.transposeValue = semitones;
    this.project.touch();
    return this;
  }

  /** Scale note velocities; factor in 0..2. */
  velocityScale(factor: number): this {
    if (!Number.isFinite(factor) || factor < 0 || factor > 2) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `velocityScale must be in 0..2, got ${factor}`,
        { details: { path: "patternClip.velocityScale" } },
      );
    }
    this.velocityScaleValue = factor;
    this.project.touch();
    return this;
  }

  /** Per-clip playback probability in 0..1 (seeded, deterministic). */
  probability(value: number): this {
    if (!Number.isFinite(value) || value < 0 || value > 1) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `probability must be in 0..1, got ${value}`,
        { details: { path: "patternClip.probability" } },
      );
    }
    this.probabilityValue = value;
    this.project.touch();
    return this;
  }

  /** Enable/disable scheduling without dropping clip data. */
  enabled(on = true): this {
    this.enabledValue = on;
    this.project.touch();
    return this;
  }

  /** Wire form; non-default options are omitted. */
  toSpec(): PatternClipSpec {
    const spec: PatternClipSpec = {
      id: this.id,
      patternId: this.pattern.id,
      trackId: this.track.id,
      startBeat: beatToWire(this.startBeat),
    };
    if (this.loopCountValue !== undefined) {
      spec.loopCount = this.loopCountValue;
    }
    if (this.lastBeatValue !== undefined) {
      spec.lastBeat = beatToWire(this.lastBeatValue);
    }
    if (this.transposeValue !== 0) {
      spec.transpose = this.transposeValue;
    }
    if (this.velocityScaleValue !== 1) {
      spec.velocityScale = this.velocityScaleValue;
    }
    if (this.probabilityValue !== 1) {
      spec.probability = this.probabilityValue;
    }
    if (!this.enabledValue) {
      spec.enabled = false;
    }
    return spec;
  }
}

/** Intermediate builder returned by `track.pattern(pattern)` before `.at()`. */
export class PatternClipDraft {
  private placed = false;

  /** @internal */
  constructor(
    private readonly project: Project,
    private readonly track: Track,
    private readonly pattern: Pattern,
  ) {}

  /**
   * Place the clip at a bar/beat position (bar from 1, beat from 0) using
   * the time signature in effect at that bar, and return the placed clip.
   */
  at(position: BarBeatPosition): PatternClip {
    if (this.placed) {
      throw new OxitoneError(ErrorCode.InvalidProject, "pattern draft is already placed");
    }
    this.placed = true;
    const startBeat = this.project.barBeatToBeats(position);
    const clip = this.project.createPatternClip(this.track, this.pattern, startBeat);
    return clip;
  }
}
