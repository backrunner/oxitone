import { lstat, opendir } from "node:fs/promises";
import { homedir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import {
  ErrorCode,
  OxitoneError,
  vst3BundleSourceSchema,
  vst3ClassListSchema,
  type Vst3BundleSource,
  type Vst3ClassList,
} from "@oxitone/protocol";
import { runHelper, type Vst3HostOptions } from "./helper.js";
import { parse } from "./validation.js";

/** Explicit directory listing only. Does not load any plugin or start a helper. */
export async function scanVst3Bundles(
  options: { directories?: string[]; signal?: AbortSignal } = {},
): Promise<string[]> {
  if (process.platform !== "darwin")
    throw new OxitoneError(ErrorCode.PluginCapabilityUnsupported, "macOS VST3 directories only");
  const roots = options.directories ?? [join(homedir(), "Library/Audio/Plug-Ins/VST3"), "/Library/Audio/Plug-Ins/VST3"];
  if (roots.length > 32 || roots.some((p) => !isAbsolute(p) || p.includes("\0")))
    throw new OxitoneError(ErrorCode.PluginConfigInvalid, "VST3 scan requires at most 32 absolute directories");
  const bundles = new Set<string>();
  let visited = 0;
  const walk = async (path: string, depth: number): Promise<void> => {
    options.signal?.throwIfAborted();
    if (++visited > 65536 || depth > 16)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 scan exceeds directory budget");
    const info = await lstat(path);
    if (!info.isDirectory() || info.isSymbolicLink()) return;
    if (path.endsWith(".vst3")) {
      bundles.add(path);
      if (bundles.size > 4096) throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 scan exceeds 4096 bundles");
      return;
    }
    for await (const child of await opendir(path)) {
      options.signal?.throwIfAborted();
      if (++visited > 65536) throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 scan exceeds entry budget");
      if (child.isDirectory() && !child.name.startsWith(".")) await walk(join(path, child.name), depth + 1);
    }
  };
  for (const path of roots) {
    try {
      await walk(resolve(path), 0);
    } catch (error) {
      if (!options.directories && (error as NodeJS.ErrnoException).code === "ENOENT") continue;
      if (error instanceof OxitoneError || options.signal?.aborted) throw error;
      throw new OxitoneError(ErrorCode.AssetUnavailable, `Cannot scan VST3 directory ${path}`, { cause: error });
    }
  }
  return [...bundles].sort();
}

/** Enumerate actual audio factory classes inside a disposable helper under the supplied policy. */
export async function listVst3Classes(source: Vst3BundleSource, host: Vst3HostOptions = {}): Promise<Vst3ClassList> {
  const selected = parse(vst3BundleSourceSchema, { ...source, bundlePath: resolve(source.bundlePath) });
  const result = parse(
    vst3ClassListSchema,
    await runHelper({ protocolVersion: 1, operation: "listClasses", source: selected }, host),
  );
  if (
    result.bundlePath !== selected.bundlePath ||
    (selected.expectedHash !== undefined && result.sha256 !== selected.expectedHash)
  )
    throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 class discovery identity mismatch");
  return result;
}
