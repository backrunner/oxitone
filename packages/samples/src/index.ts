import { isAbsolute, relative, resolve, sep } from "node:path";
import { ErrorCode, inspectSampleRequestSchema, OxitoneError, type SampleInfo } from "@oxitone/protocol";
import { inspectSample } from "oxitone";

export { inspectSample } from "oxitone";
export type { SampleInfo } from "@oxitone/protocol";

export interface ImportSampleOptions {
  /** Resolve input against this directory and emit an asset URI relative to it. */
  assetBaseDir?: string;
}

export type SampleProvenance = Pick<SampleInfo,
  "sourceChannels" | "sourceBitDepth" | "decoder" | "channelLayoutAction">;

/** Compatible with Project.addSample; provenance is kept outside the project snapshot. */
export interface ImportedSample extends Pick<SampleInfo, "sha256" | "format" | "sampleRate" | "channels"> {
  assetUri: string;
  frames: bigint;
  provenance: SampleProvenance;
}

/**
 * Synchronously decode a local file in Rust and return an asset descriptor.
 * Does not write/copy files or retain PCM. Prepare verifies the hash and decodes again.
 * Relative paths use assetBaseDir when supplied, otherwise the current directory.
 * With assetBaseDir, the source must be inside that directory; render with the same base.
 */
export function importSample(path: string, options: ImportSampleOptions = {}): ImportedSample {
  validatePath(path, "path");
  if (options.assetBaseDir !== undefined) validatePath(options.assetBaseDir, "assetBaseDir");
  const base = options.assetBaseDir === undefined ? undefined : resolve(options.assetBaseDir);
  const absolute = base === undefined ? resolve(path) : resolve(base, path);
  const assetUri = base === undefined ? absolute : relative(base, absolute).split(sep).join("/");
  if (base !== undefined && (assetUri === ".." || assetUri.startsWith("../") || isAbsolute(assetUri))) {
    throw new OxitoneError(ErrorCode.InvalidProject, "sample must be inside assetBaseDir", {
      details: { path: absolute },
    });
  }
  const info = inspectSample(absolute);
  return {
    assetUri, sha256: info.sha256, format: info.format, sampleRate: info.sampleRate,
    channels: info.channels, frames: BigInt(info.frames),
    provenance: {
      sourceChannels: info.sourceChannels, decoder: info.decoder,
      channelLayoutAction: info.channelLayoutAction,
      ...(info.sourceBitDepth === undefined ? {} : { sourceBitDepth: info.sourceBitDepth }),
    },
  };
}

function validatePath(path: string, field: string): void {
  if (!inspectSampleRequestSchema.shape.path.safeParse(path).success) {
    throw new OxitoneError(ErrorCode.InvalidProject, `${field} must be a nonempty local path without NUL`, {
      details: { path: field },
    });
  }
}
