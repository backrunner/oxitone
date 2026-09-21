import { constants } from "node:fs";
import { link, mkdir, mkdtemp, open, rm } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { canonicalEncode, ErrorCode, OxitoneError, vst3PresetSchema, type Vst3Preset } from "@oxitone/protocol";
import { parse } from "./validation.js";

const MAX_BYTES = 8 * 1024 * 1024;
export function parseVst3Preset(value: unknown): Vst3Preset {
  if (typeof value === "object" && value !== null && "formatVersion" in value && value.formatVersion !== 1)
    throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "Unsupported VST3 preset; preserve the original file");
  const preset = parse(vst3PresetSchema, value);
  if (!isAbsolute(preset.source.bundlePath))
    throw new OxitoneError(ErrorCode.InvalidProject, "VST3 preset requires an absolute local bundle path");
  return preset;
}
function local(path: string): string {
  if (!path || path.length > 4096 || path.includes("\0"))
    throw new OxitoneError(ErrorCode.InvalidProject, "Invalid VST3 preset path");
  return resolve(path);
}
function failure(error: unknown): never {
  if (OxitoneError.isOxitoneError(error)) throw error;
  throw new OxitoneError(ErrorCode.AssetUnavailable, "Cannot read or publish VST3 preset (new files only)", {
    cause: error,
  });
}

/** Persist data only. Never loads native code and never replaces an existing file. */
export async function saveVst3Preset(path: string, value: Vst3Preset): Promise<void> {
  const destination = local(path),
    preset = parseVst3Preset(value);
  const bytes = Buffer.from(canonicalEncode(preset));
  if (bytes.length > MAX_BYTES) throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 preset exceeds 8 MiB");
  let temporary: string | undefined;
  try {
    await mkdir(dirname(destination), { recursive: true });
    temporary = await mkdtemp(join(dirname(destination), ".oxitone-vst3-preset-"));
    const staged = join(temporary, "preset.json"),
      file = await open(staged, "wx", 0o600);
    try {
      await file.writeFile(bytes);
      await file.sync();
    } finally {
      await file.close();
    }
    await link(staged, destination);
    const directory = await open(dirname(destination), "r");
    try {
      await directory.sync();
    } finally {
      await directory.close();
    }
  } catch (error) {
    failure(error);
  } finally {
    if (temporary) await rm(temporary, { recursive: true, force: true });
  }
}

/** Bounded data load. Unsupported/corrupt files are left byte-for-byte intact. */
export async function loadVst3Preset(path: string): Promise<Vst3Preset> {
  try {
    const file = await open(local(path), constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
    try {
      const stat = await file.stat();
      if (!stat.isFile()) throw new OxitoneError(ErrorCode.AssetUnavailable, "VST3 preset must be a regular file");
      if (stat.size > MAX_BYTES) throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 preset exceeds 8 MiB");
      // One sentinel byte detects growth without allocating the full budget for every small preset.
      const bytes = Buffer.alloc(stat.size + 1);
      let size = 0;
      while (size < bytes.length) {
        const { bytesRead } = await file.read(bytes, size, bytes.length - size, null);
        if (!bytesRead) break;
        size += bytesRead;
      }
      if (size > MAX_BYTES) throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 preset exceeds 8 MiB");
      if (size !== stat.size)
        throw new OxitoneError(ErrorCode.SourceChanged, "VST3 preset changed while reading; retry the load");
      return parseVst3Preset(JSON.parse(bytes.subarray(0, size).toString("utf8")));
    } finally {
      await file.close();
    }
  } catch (error) {
    failure(error);
  }
}
