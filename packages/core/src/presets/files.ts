import { createHash, randomUUID } from "node:crypto";
import { open, readFile, realpath, rename, unlink } from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { canonicalEncode, ErrorCode, OxitoneError, type Preset } from "@oxitone/protocol";
import { parsePreset, validatePreset, type PresetRuntimeOptions } from "./preset.js";

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
function fileError(error: unknown, path: string): never {
  if (OxitoneError.isOxitoneError(error)) throw error;
  throw new OxitoneError(
    error instanceof SyntaxError ? ErrorCode.InvalidProject : ErrorCode.AssetUnavailable,
    error instanceof Error ? error.message : "preset file operation failed",
    { details: { path } },
  );
}
function localPath(path: string): string {
  if (!path || path.includes("\0"))
    throw new OxitoneError(ErrorCode.InvalidProject, "preset path must be nonempty without NUL");
  return resolve(path);
}
function inside(base: string, path: string): string {
  const uri = relative(base, path).split(sep).join("/");
  if (isAbsolute(uri) || uri === ".." || uri.startsWith("../")) {
    throw new OxitoneError(ErrorCode.InvalidProject, "preset assets must be inside its directory");
  }
  return uri;
}

/** Write only canonical metadata; sample bytes remain external and hash-verified. */
export async function savePreset(value: Preset, path: string, options: PresetRuntimeOptions = {}): Promise<void> {
  const destination = localPath(path);
  const temp = `${destination}.${randomUUID()}.tmp`;
  try {
    const preset = validatePreset(value, options);
    const base = await realpath(dirname(destination));
    for (const sample of preset.samples) {
      const source = await realpath(resolve(options.assetBaseDir ?? process.cwd(), sample.assetUri));
      sample.assetUri = inside(base, source);
      if (hash(await readFile(source)) !== sample.sha256)
        throw new OxitoneError(ErrorCode.AssetUnavailable, "preset sample hash mismatch");
    }
    const file = await open(temp, "wx");
    try {
      await file.writeFile(canonicalEncode(preset));
      await file.sync();
    } finally {
      await file.close();
    }
    await rename(temp, destination);
    const directory = await open(base, "r");
    try {
      await directory.sync();
    } finally {
      await directory.close();
    }
  } catch (error) {
    fileError(error, destination);
  } finally {
    await unlink(temp).catch(() => {});
  }
}

export interface LoadedPreset {
  preset: Preset;
  assetBaseDir: string;
}
/** Load portable metadata and validate all plugins/resources before returning. */
export async function loadPreset(path: string, options: PresetRuntimeOptions = {}): Promise<LoadedPreset> {
  const source = localPath(path);
  try {
    const preset = parsePreset(JSON.parse(await readFile(source, "utf8")));
    const assetBaseDir = await realpath(dirname(source));
    for (const sample of preset.samples) {
      if (isAbsolute(sample.assetUri) || sample.assetUri.includes("\\") || sample.assetUri.split("/").includes("..")) {
        throw new OxitoneError(ErrorCode.InvalidProject, "preset asset URI must be relative and contained");
      }
      const asset = await realpath(resolve(assetBaseDir, sample.assetUri));
      inside(assetBaseDir, asset);
      if (hash(await readFile(asset)) !== sample.sha256)
        throw new OxitoneError(ErrorCode.AssetUnavailable, "preset sample hash mismatch");
    }
    return { preset: validatePreset(preset, { ...options, assetBaseDir }), assetBaseDir };
  } catch (error) {
    fileError(error, source);
  }
}
