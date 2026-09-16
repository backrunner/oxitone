import {
  beatToWire,
  beatFromWire,
  sampleClipSpecSchema,
  ErrorCode,
  OxitoneError,
  type EntityId,
  type SampleClipSpec,
} from "@oxitone/protocol";
import { parseAuthoring } from "../authoring-validation.js";
import type { BarBeatPosition } from "../timing/time-signature.js";
import type { Project } from "../project/project.js";
import type { Track } from "./track.js";
import type { Sample } from "./sample.js";

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
  constructor(
    private readonly project: Project,
    private trackValue: Track,
    readonly sample: Sample,
    id: EntityId,
    start: BarBeatPosition,
    options: SampleClipOptions = {},
  ) {
    if (options.durationBeats !== undefined) this.ensurePositive(options.durationBeats, "sampleClip.durationBeats");
    if (options.loop !== undefined && options.loop.count !== undefined && options.loop.lastBeat !== undefined) {
      throw new OxitoneError(ErrorCode.InvalidProject, "loop count and lastBeat are mutually exclusive", {
        details: { path: "sampleClip.loop" },
      });
    }
    if (options.loop !== undefined && (!Number.isFinite(options.loop.lengthBeats) || options.loop.lengthBeats <= 0)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "loop lengthBeats must be finite and > 0", {
        details: { path: "sampleClip.loop.lengthBeats" },
      });
    }
    this.spec = parseAuthoring(
      sampleClipSpecSchema,
      {
        ...options,
        id,
        sampleId: sample.id,
        trackId: trackValue.id,
        startBeat: beatToWire(project.barBeatToBeats(start)),
        ...(options.loop === undefined
          ? {}
          : {
              loop: {
                ...options.loop,
                lengthBeats: beatToWire(options.loop.lengthBeats),
                ...(options.loop.startBeat === undefined ? {} : { startBeat: beatToWire(options.loop.startBeat) }),
                ...(options.loop.lastBeat === undefined ? {} : { lastBeat: beatToWire(options.loop.lastBeat) }),
              },
            }),
        ...(options.durationBeats === undefined ? {} : { durationBeats: beatToWire(options.durationBeats) }),
      },
      "sampleClip",
    );
  }

  /** Current owner; use relocate() to move a clip and update Track membership. */
  get track(): Track {
    return this.trackValue;
  }

  get id(): EntityId {
    return this.spec.id;
  }
  relocate(track: Track, startBeat: number): void {
    if (!this.project.tracks.includes(track) || !Number.isFinite(startBeat) || startBeat < 0)
      throw new OxitoneError(ErrorCode.InvalidProject, "Invalid sample destination");
    this.update({ trackId: track.id, startBeat: beatToWire(startBeat) });
    if (this.track !== track) {
      this.track.detachSampleClip(this);
      track.attachSampleClip(this);
      this.trackValue = track;
    }
  }

  /** @internal Restore wire timing; bar lookup is only used by subsequent fitBars calls. */
  static fromSpec(project: Project, track: Track, sample: Sample, input: SampleClipSpec): SampleClip {
    const spec = parseAuthoring(sampleClipSpecSchema, input, "sampleClip");
    if (
      spec.durationBeats?.numerator === 0 ||
      spec.loop?.lengthBeats.numerator === 0 ||
      (spec.loop?.count !== undefined && spec.loop.lastBeat !== undefined)
    ) {
      throw new OxitoneError(ErrorCode.InvalidProject, "invalid sample clip duration or loop");
    }
    const clip = new SampleClip(project, track, sample, spec.id, project.beatsToBarBeat(beatFromWire(spec.startBeat)));
    clip.spec = spec;
    return clip;
  }
  get startBeat(): number {
    return Number(this.spec.startBeat.numerator) / Number(this.spec.startBeat.denominator);
  }
  get durationBeats(): number | undefined {
    const b = this.spec.durationBeats;
    return b === undefined ? undefined : Number(b.numerator) / Number(b.denominator);
  }
  get gain(): number {
    return this.spec.gain ?? 1;
  }
  set gain(value: number) {
    this.update({ gain: value });
  }
  get pan(): number {
    return this.spec.pan ?? 0;
  }
  set pan(value: number) {
    this.update({ pan: value });
  }
  get rate(): number {
    return this.spec.rate ?? 1;
  }
  set rate(value: number) {
    this.update({ rate: value });
  }
  get tempoSync(): "off" | "stretch" | "repitch" {
    return this.spec.tempoSync ?? "off";
  }
  set tempoSync(value: "off" | "stretch" | "repitch") {
    this.update({ tempoSync: value });
  }
  get enabled(): boolean {
    return this.spec.enabled !== false;
  }
  set enabled(value: boolean) {
    this.update({ enabled: value });
  }
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
    return this.fitBeats(bars * this.project.beatsPerBarAt(this.project.beatsToBarBeat(this.startBeat).bar));
  }

  fitToContent(): this {
    const edits = this.sample.edits;
    const frames = BigInt(edits?.endFrame ?? this.sample.frames) - BigInt(edits?.startFrame ?? "0");
    const seconds = Number(frames) / this.sample.sampleRate;
    const beats =
      this.sample.musicalLengthBeats ??
      (this.track.tempo === undefined
        ? this.project.beatsForSeconds(this.startBeat, seconds)
        : (seconds * this.track.tempo) / 60);
    return this.fitBeats(beats);
  }

  toSpec(): SampleClipSpec {
    return structuredClone(this.spec);
  }
  private ensurePositive(value: number, path: string): void {
    if (!Number.isFinite(value) || value <= 0)
      throw new OxitoneError(ErrorCode.InvalidProject, `${path} must be finite and > 0`, { details: { path } });
  }
  private update(patch: Partial<SampleClipSpec>): void {
    this.project.assertMutable();
    this.spec = parseAuthoring(sampleClipSpecSchema, { ...this.spec, ...patch }, "sampleClip");
    this.project.touch();
  }
}
