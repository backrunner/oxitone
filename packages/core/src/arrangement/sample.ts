import {
  beatToWire,
  beatFromWire,
  frameToWire,
  sampleRefSchema,
  ErrorCode,
  OxitoneError,
  type EntityId,
  type SampleEditSpec,
  type SampleRef,
} from "@oxitone/protocol";
import { parseAuthoring } from "../authoring-validation.js";

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
  provenance?: SampleRef["provenance"];
}

/** Immutable audio asset reference; decoding and editing happen in Rust prepare. */
export class Sample {
  readonly id: EntityId;
  private spec: SampleRef;

  constructor(id: EntityId, options: SampleOptions) {
    this.id = id;
    const frames =
      typeof options.frames === "bigint"
        ? options.frames
        : Number.isSafeInteger(options.frames)
          ? BigInt(options.frames)
          : undefined;
    if (frames === undefined || frames <= 0n || frames > 0xffff_ffff_ffff_ffffn) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "sample frames must be a positive u64 (use bigint above Number.MAX_SAFE_INTEGER)",
        { details: { path: "sample.frames" } },
      );
    }
    this.spec = parseAuthoring(
      sampleRefSchema,
      {
        ...options,
        id,
        frames: frameToWire(frames),
        ...(options.musicalLengthBeats === undefined
          ? {}
          : { musicalLengthBeats: beatToWire(options.musicalLengthBeats) }),
      },
      "sample",
    );
    const start = BigInt(this.spec.edits?.startFrame ?? "0");
    const end = BigInt(this.spec.edits?.endFrame ?? frames);
    if (start >= end || end > frames) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "sample trim must satisfy 0 <= startFrame < endFrame <= frames",
        {
          details: { path: "sample.edits" },
        },
      );
    }
    if (options.musicalLengthBeats !== undefined && options.musicalLengthBeats <= 0) {
      throw new OxitoneError(ErrorCode.InvalidProject, "musicalLengthBeats must be positive", {
        details: { path: "sample.musicalLengthBeats" },
      });
    }
  }

  get assetUri(): string {
    return this.spec.assetUri;
  }

  /** Restore an immutable resource without rounding its musical length through a double. */
  static fromSpec(input: SampleRef): Sample {
    const spec = parseAuthoring(sampleRefSchema, input, "sample");
    const sample = new Sample(spec.id, {
      assetUri: spec.assetUri,
      sha256: spec.sha256,
      format: spec.format,
      sampleRate: spec.sampleRate,
      channels: spec.channels,
      frames: BigInt(spec.frames),
      ...(spec.edits === undefined ? {} : { edits: spec.edits }),
      ...(spec.musicalLengthBeats === undefined ? {} : { musicalLengthBeats: beatFromWire(spec.musicalLengthBeats) }),
    });
    sample.spec = spec;
    return sample;
  }
  get sampleRate(): number {
    return this.spec.sampleRate;
  }
  get channels(): 1 | 2 {
    return this.spec.channels;
  }
  get frames(): bigint {
    return BigInt(this.spec.frames);
  }
  get format(): SampleRef["format"] {
    return this.spec.format;
  }
  get provenance(): SampleRef["provenance"] {
    return structuredClone(this.spec.provenance);
  }
  get musicalLengthBeats(): number | undefined {
    return this.spec.musicalLengthBeats === undefined
      ? undefined
      : Number(this.spec.musicalLengthBeats.numerator) / Number(this.spec.musicalLengthBeats.denominator);
  }
  get edits(): SampleEditSpec | undefined {
    return this.spec.edits === undefined ? undefined : structuredClone(this.spec.edits);
  }
  toSpec(): SampleRef {
    return structuredClone(this.spec);
  }
}
