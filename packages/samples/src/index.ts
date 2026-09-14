import { isAbsolute, relative, resolve, sep } from "node:path";
import {
  ErrorCode,
  inspectSampleRequestSchema,
  OxitoneError,
  type SampleInfo,
  type SampleProvenance,
} from "@oxitone/protocol";
import { cacheSample, inspectSample } from "@oxitone/native";

export { inspectSample, cacheSample } from "@oxitone/native";
export type { SampleInfo, CachedSampleInfo, SampleProvenance } from "@oxitone/protocol";

export interface ImportSampleOptions {
  /** Resolve input against this directory and emit an asset URI relative to it. */
  assetBaseDir?: string;
  /** Publish a normalized float32 WAV here; relative directories use assetBaseDir or cwd. */
  cacheDir?: string;
}

/** Compatible with Project.addSample, including persistent source provenance. */
export interface ImportedSample extends Pick<SampleInfo, "sha256" | "format" | "sampleRate" | "channels"> {
  assetUri: string;
  frames: bigint;
  provenance: SampleProvenance;
}

/**
 * Synchronously decode a local file in Rust and return an asset descriptor.
 * Without cacheDir this is read-only. With it, Rust publishes an immutable WAV.
 * Prepare verifies the returned asset hash. PCM never crosses the native boundary.
 * Relative paths use assetBaseDir when supplied, otherwise the current directory.
 * With assetBaseDir, the returned asset must be inside it; caching permits external sources.
 */
export function importSample(path: string, options: ImportSampleOptions = {}): ImportedSample {
  validatePath(path, "path");
  if (options.assetBaseDir !== undefined) validatePath(options.assetBaseDir, "assetBaseDir");
  if (options.cacheDir !== undefined) validatePath(options.cacheDir, "cacheDir");
  const base = options.assetBaseDir === undefined ? undefined : resolve(options.assetBaseDir);
  const absolute = base === undefined ? resolve(path) : resolve(base, path);
  if (options.cacheDir !== undefined) {
    const directory = resolve(base ?? process.cwd(), options.cacheDir);
    relativeAsset(directory, base); // Reject an escaping destination before writing anything.
    const cached = cacheSample(absolute, directory);
    return {
      assetUri: relativeAsset(cached.path, base),
      sha256: cached.sha256,
      format: cached.format,
      sampleRate: cached.sampleRate,
      channels: cached.channels,
      frames: BigInt(cached.frames),
      provenance: cached.provenance,
    };
  }
  const assetUri = relativeAsset(absolute, base);
  const info = inspectSample(absolute);
  return {
    assetUri,
    sha256: info.sha256,
    format: info.format,
    sampleRate: info.sampleRate,
    channels: info.channels,
    frames: BigInt(info.frames),
    provenance: {
      sourceSha256: info.sha256,
      sourceFormat: info.format,
      sourceSampleRate: info.sampleRate,
      sourceChannels: info.sourceChannels,
      decoder: info.decoder,
      channelLayoutAction: info.channelLayoutAction,
      ...(info.sourceBitDepth === undefined ? {} : { sourceBitDepth: info.sourceBitDepth }),
    },
  };
}

function relativeAsset(absolute: string, base: string | undefined): string {
  const assetUri = base === undefined ? absolute : relative(base, absolute).split(sep).join("/");
  if (base !== undefined && (assetUri === ".." || assetUri.startsWith("../") || isAbsolute(assetUri))) {
    throw new OxitoneError(ErrorCode.InvalidProject, "sample must be inside assetBaseDir", {
      details: { path: absolute },
    });
  }
  return assetUri;
}

function validatePath(path: string, field: string): void {
  if (!inspectSampleRequestSchema.shape.path.safeParse(path).success) {
    throw new OxitoneError(ErrorCode.InvalidProject, `${field} must be a nonempty local path without NUL`, {
      details: { path: field },
    });
  }
}
