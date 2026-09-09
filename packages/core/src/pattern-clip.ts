import { beatToWire, beatFromWire, patternClipSpecSchema, ErrorCode, OxitoneError, type PatternClipSpec } from "@oxitone/protocol";
import { parseAuthoring } from "./authoring-validation.js";
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
  track: Track;
  startBeat: number;
  private readonly project: Project;
  private loopCountValue: number | undefined;
  private lastBeatValue: number | undefined;
  private transposeValue = 0;
  private velocityScaleValue = 1;
  private probabilityValue = 1;
  private enabledValue = true;
  private restoredSpec?: PatternClipSpec;
  private durationValue: PatternClipSpec["durationBeats"] | undefined;

  /** @internal Use `track.pattern(pattern).at(...)` instead. */
  constructor(project: Project, track: Track, pattern: Pattern, id: string, startBeat: number) {
    this.project = project;
    this.track = track;
    this.pattern = pattern;
    this.id = id;
    this.startBeat = startBeat;
  }

  /** @internal Restore exact wire positions and explicit defaults. */
  static fromSpec(project: Project, track: Track, pattern: Pattern, input: PatternClipSpec): PatternClip {
    const spec = parseAuthoring(patternClipSpecSchema, input, "patternClip");
    if (spec.durationBeats?.numerator === 0 || (spec.lastBeat !== undefined &&
      BigInt(spec.lastBeat.numerator) * BigInt(spec.startBeat.denominator) <=
      BigInt(spec.startBeat.numerator) * BigInt(spec.lastBeat.denominator))) {
      throw new OxitoneError(ErrorCode.InvalidProject, "invalid pattern clip duration or end");
    }
    const clip = new PatternClip(project, track, pattern, spec.id, beatFromWire(spec.startBeat));
    clip.loopCountValue = spec.loopCount;
    clip.lastBeatValue = spec.lastBeat === undefined ? undefined : beatFromWire(spec.lastBeat);
    clip.transposeValue = spec.transpose ?? 0;
    clip.velocityScaleValue = spec.velocityScale ?? 1;
    clip.probabilityValue = spec.probability ?? 1;
    clip.enabledValue = spec.enabled ?? true;
    clip.durationValue = spec.durationBeats;
    clip.restoredSpec = spec;
    return clip;
  }

  get durationBeats(): number | undefined {
    return this.durationValue === undefined ? undefined : beatFromWire(this.durationValue);
  }

  relocate(track: Track, beat: number): void {
    this.project.assertMutable();
    if (!this.project.tracks.includes(track) || !Number.isFinite(beat) || beat < 0) throw new OxitoneError(ErrorCode.InvalidProject, "Invalid clip destination");
    const routes = this.track.channelIds;
    for (const id of routes) {
      const channel = this.project.channels.find(candidate => candidate.id === id);
      if (channel) track.use(channel);
    }
    if (this.track !== track) { this.track.detachClip(this); track.attachClip(this); }
    if (this.lastBeatValue !== undefined) {
      this.lastBeatValue += beat - this.startBeat;
      if (this.restoredSpec) this.restoredSpec.lastBeat = beatToWire(this.lastBeatValue);
    }
    this.track = track; this.startBeat = beat;
    if (this.restoredSpec) { this.restoredSpec.trackId = track.id; this.restoredSpec.startBeat = beatToWire(beat); }
    this.project.touch();
  }

  /** Explicit clip length; undefined uses the pattern/loop boundary. */
  set durationBeats(value: number | undefined) {
    if (value !== undefined && (!Number.isFinite(value) || value <= 0)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "pattern clip duration must be finite and positive");
    }
    const duration = value === undefined ? undefined : beatToWire(value);
    this.project.touch();
    this.durationValue = duration;
    if (this.restoredSpec !== undefined) this.restoredSpec.durationBeats = duration;
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
    this.project.assertMutable();
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
    if (this.restoredSpec !== undefined) this.restoredSpec.loopCount = count;
    this.project.touch();
    return this;
  }

  /** Exclusive end position (bar/beat) for looping; trims trailing notes. */
  last(position: BarBeatPosition): this {
    this.project.assertMutable();
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
    if (this.restoredSpec !== undefined) this.restoredSpec.lastBeat = beatToWire(beat);
    this.project.touch();
    return this;
  }

  /** Transpose all notes by `semitones` (integer). */
  transpose(semitones: number): this {
    this.project.assertMutable();
    if (!Number.isInteger(semitones)) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `transpose must be an integer, got ${semitones}`,
        { details: { path: "patternClip.transpose" } },
      );
    }
    this.transposeValue = semitones;
    if (this.restoredSpec !== undefined) this.restoredSpec.transpose = semitones;
    this.project.touch();
    return this;
  }

  /** Scale note velocities; factor in 0..2. */
  velocityScale(factor: number): this {
    this.project.assertMutable();
    if (!Number.isFinite(factor) || factor < 0 || factor > 2) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `velocityScale must be in 0..2, got ${factor}`,
        { details: { path: "patternClip.velocityScale" } },
      );
    }
    this.velocityScaleValue = factor;
    if (this.restoredSpec !== undefined) this.restoredSpec.velocityScale = factor;
    this.project.touch();
    return this;
  }

  /** Per-clip playback probability in 0..1 (seeded, deterministic). */
  probability(value: number): this {
    this.project.assertMutable();
    if (!Number.isFinite(value) || value < 0 || value > 1) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `probability must be in 0..1, got ${value}`,
        { details: { path: "patternClip.probability" } },
      );
    }
    this.probabilityValue = value;
    if (this.restoredSpec !== undefined) this.restoredSpec.probability = value;
    this.project.touch();
    return this;
  }

  /** Enable/disable scheduling without dropping clip data. */
  enabled(on = true): this {
    this.project.assertMutable();
    if (typeof on !== "boolean") throw new OxitoneError(ErrorCode.InvalidProject, "enabled must be a boolean");
    this.enabledValue = on;
    if (this.restoredSpec !== undefined) {
      if (on) delete this.restoredSpec.enabled; else this.restoredSpec.enabled = false;
    }
    this.project.touch();
    return this;
  }

  /** Wire form; non-default options are omitted. */
  toSpec(): PatternClipSpec {
    if (this.restoredSpec !== undefined) return structuredClone(this.restoredSpec);
    const spec: PatternClipSpec = {
      id: this.id,
      patternId: this.pattern.id,
      trackId: this.track.id,
      startBeat: beatToWire(this.startBeat),
    };
    if (this.durationValue !== undefined) spec.durationBeats = { ...this.durationValue };
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
    const startBeat = this.project.barBeatToBeats(position);
    const clip = this.project.createPatternClip(this.track, this.pattern, startBeat);
    this.placed = true;
    return clip;
  }
}
