import {
  beatToWire,
  frameToWire,
  sampleClipSpecSchema,
  sampleRefSchema,
  ErrorCode,
  OxitoneError,
  type EntityId,
  type SampleClipSpec,
  type SampleEditSpec,
  type SampleRef,
} from "@oxitone/protocol";
import { parseAuthoring } from "./authoring-validation.js";
import type { BarBeatPosition } from "./time-signature.js";
import type { Project } from "./project.js";
import type { Track } from "./track.js";

export interface SampleOptions {
  id?: EntityId;
  assetUri: string;
  sha256: string;
  format: SampleRef["format"];
  sampleRate: number;
  channels: 1 | 2;
  frames: bigint | number;
  edits?: SampleEditSpec;
  musicalLengthBeats?: number;
}

/** Immutable audio asset reference; decoding and editing happen in Rust prepare. */
export class Sample {
  readonly id: EntityId;
  private readonly spec: SampleRef;

  constructor(id: EntityId, options: SampleOptions) {
    this.id = id;
    const frames = typeof options.frames === "bigint" ? options.frames :
      (Number.isSafeInteger(options.frames) ? BigInt(options.frames) : undefined);
    if (frames === undefined || frames <= 0n || frames > 0xffff_ffff_ffff_ffffn) {
      throw new OxitoneError(ErrorCode.InvalidProject, "sample frames must be a positive u64 (use bigint above Number.MAX_SAFE_INTEGER)", { details: { path: "sample.frames" } });
    }
    this.spec = parseAuthoring(sampleRefSchema, {
      ...options,
      id,
      frames: frameToWire(frames),
      ...(options.musicalLengthBeats === undefined
        ? {}
        : { musicalLengthBeats: beatToWire(options.musicalLengthBeats) }),
    }, "sample");
    const start = BigInt(this.spec.edits?.startFrame ?? "0");
    const end = BigInt(this.spec.edits?.endFrame ?? frames);
    if (start >= end || end > frames) {
      throw new OxitoneError(ErrorCode.InvalidProject, "sample trim must satisfy 0 <= startFrame < endFrame <= frames", {
        details: { path: "sample.edits" },
      });
    }
    if (options.musicalLengthBeats !== undefined && options.musicalLengthBeats <= 0) {
      throw new OxitoneError(ErrorCode.InvalidProject, "musicalLengthBeats must be positive", {
        details: { path: "sample.musicalLengthBeats" },
      });
    }
  }

  get assetUri(): string { return this.spec.assetUri; }
  get sampleRate(): number { return this.spec.sampleRate; }
  get channels(): 1 | 2 { return this.spec.channels; }
  get frames(): bigint { return BigInt(this.spec.frames); }
  get format(): SampleRef["format"] { return this.spec.format; }
  get musicalLengthBeats(): number | undefined {
    return this.spec.musicalLengthBeats === undefined
      ? undefined
      : Number(this.spec.musicalLengthBeats.numerator) /
        Number(this.spec.musicalLengthBeats.denominator);
  }
  get edits(): SampleEditSpec | undefined {
    return this.spec.edits === undefined ? undefined : structuredClone(this.spec.edits);
  }
  toSpec(): SampleRef { return structuredClone(this.spec); }
}

export interface SampleClipOptions {
  durationBeats?: number;
  gain?: number;
  pan?: number;
  rate?: number;
  loop?: { startBeat?: number; lengthBeats: number; count?: number; lastBeat?: number };
  tempoSync?: "off" | "stretch" | "repitch";
  stretchAlgorithm?: string;
  enabled?: boolean;
}

/** A sample placed on a Track; all positions and lengths stay in beats. */
export class SampleClip {
  private spec: SampleClipSpec;
  private readonly startBar: number;
  constructor(
    private readonly project: Project,
    readonly track: Track,
    readonly sample: Sample,
    id: EntityId,
    start: BarBeatPosition,
    options: SampleClipOptions = {},
  ) {
    this.startBar = start.bar;
    if (options.durationBeats !== undefined) this.ensurePositive(options.durationBeats, "sampleClip.durationBeats");
    if (options.loop !== undefined && options.loop.count !== undefined && options.loop.lastBeat !== undefined) {
      throw new OxitoneError(ErrorCode.InvalidProject, "loop count and lastBeat are mutually exclusive", { details: { path: "sampleClip.loop" } });
    }
    if (options.loop !== undefined && (!Number.isFinite(options.loop.lengthBeats) || options.loop.lengthBeats <= 0)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "loop lengthBeats must be finite and > 0", { details: { path: "sampleClip.loop.lengthBeats" } });
    }
    this.spec = parseAuthoring(sampleClipSpecSchema, {
      ...options, id, sampleId: sample.id, trackId: track.id,
      startBeat: beatToWire(project.barBeatToBeats(start)),
      ...(options.loop === undefined ? {} : { loop: { ...options.loop, lengthBeats: beatToWire(options.loop.lengthBeats), ...(options.loop.startBeat === undefined ? {} : { startBeat: beatToWire(options.loop.startBeat) }), ...(options.loop.lastBeat === undefined ? {} : { lastBeat: beatToWire(options.loop.lastBeat) }) } }),
      ...(options.durationBeats === undefined ? {} : { durationBeats: beatToWire(options.durationBeats) }),
    }, "sampleClip");
  }

  get id(): EntityId { return this.spec.id; }
  get startBeat(): number {
    return Number(this.spec.startBeat.numerator) / Number(this.spec.startBeat.denominator);
  }
  get durationBeats(): number | undefined {
    const b = this.spec.durationBeats;
    return b === undefined ? undefined : Number(b.numerator) / Number(b.denominator);
  }
  get gain(): number { return this.spec.gain ?? 1; }
  set gain(value: number) { this.update({ gain: value }); }
  get pan(): number { return this.spec.pan ?? 0; }
  set pan(value: number) { this.update({ pan: value }); }
  get rate(): number { return this.spec.rate ?? 1; }
  set rate(value: number) { this.update({ rate: value }); }
  get tempoSync(): "off" | "stretch" | "repitch" { return this.spec.tempoSync ?? "off"; }
  set tempoSync(value: "off" | "stretch" | "repitch") { this.update({ tempoSync: value }); }
  get enabled(): boolean { return this.spec.enabled !== false; }
  set enabled(value: boolean) { this.update({ enabled: value }); }
  get loop(): SampleClipSpec["loop"] {
    return this.spec.loop === undefined ? undefined : structuredClone(this.spec.loop);
  }

  fitBeats(beats: number): this {
    this.ensurePositive(beats, "sampleClip.durationBeats");
    this.update({ durationBeats: beatToWire(beats) });
    return this;
  }

  fitBars(bars: number): this {
    this.ensurePositive(bars, "sampleClip.fitBars");
    return this.fitBeats(bars * this.project.beatsPerBarAt(this.startBar));
  }

  fitToContent(): this {
    const edits = this.sample.edits;
    const frames = BigInt(edits?.endFrame ?? this.sample.frames) - BigInt(edits?.startFrame ?? "0");
    const beats = this.sample.musicalLengthBeats ??
      this.project.beatsForSeconds(this.startBeat, Number(frames) / this.sample.sampleRate);
    return this.fitBeats(beats);
  }

  toSpec(): SampleClipSpec { return structuredClone(this.spec); }
  private ensurePositive(value: number, path: string): void {
    if (!Number.isFinite(value) || value <= 0) throw new OxitoneError(ErrorCode.InvalidProject, `${path} must be finite and > 0`, { details: { path } });
  }
  private update(patch: Partial<SampleClipSpec>): void {
    this.spec = parseAuthoring(sampleClipSpecSchema, { ...this.spec, ...patch }, "sampleClip");
    this.project.touch();
  }
}
