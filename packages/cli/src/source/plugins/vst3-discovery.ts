import { lstat, realpath } from "node:fs/promises";
import { basename, isAbsolute, relative, resolve } from "node:path";
import {
  ErrorCode,
  OxitoneError,
  type Vst3PackagePlugin,
  type Vst3Source,
  type Vst3Info,
  type ParameterSpec,
} from "@oxitone/protocol";
import type { SourceRead } from "../files/read-set.js";
import { sourceHash } from "../syntax/program.js";
import type { DiscoveredPlugin } from "./plugin-discovery.js";

export function vst3ParameterSpecs(info: Vst3Info): ParameterSpec[] {
  return info.parameters
    .filter((p) => !p.readOnly)
    .map((p) => ({
      id: String(p.id),
      label: p.name,
      unit: "normalized",
      min: 0,
      max: 1,
      default: p.default,
      smoothing: "none",
      rate: "control",
      automation: p.canAutomate,
      mapping: "linear",
    }));
}

export function vst3PluginKind(info: Vst3Info): "instrument" | "effect" | "unknown" {
  if (info.audioBuses.inputs.length === 0 && info.audioBuses.outputs.length === 0 && info.noteOutput)
    return "instrument";
  const categories = info.category.split("|").map((category) => category.trim().toLowerCase());
  if (categories.includes("instrument")) return "instrument";
  if (categories.includes("fx")) return "effect";
  if (info.inputChannels > 0) return "effect";
  return info.noteInput ? "instrument" : "unknown";
}

/** Explicit bundle/class identity; this operation never loads native code. */
export function localVst3(source: Vst3Source): DiscoveredPlugin {
  return {
    reads: [],
    vst3Source: source,
    entry: {
      handle: sourceHash(JSON.stringify([source.bundlePath, source.classId.toLowerCase(), source.expectedHash])),
      pluginId: `vst3.${source.classId.toLowerCase()}`,
      pluginVersion: "unknown",
      displayName: basename(source.bundlePath),
      vendor: "",
      kind: "unknown",
      source: "vst3",
      availability: process.platform === "darwin" ? "available" : "unsupported",
      validation: "unverified",
      parameters: [],
      usages: [],
      vst3: { bundlePath: source.bundlePath, classId: source.classId.toLowerCase(), parameters: [] },
      ...(source.expectedHash ? { sha256: source.expectedHash } : {}),
    },
  };
}

export async function discoverVst3PackagePlugin(
  plugin: Vst3PackagePlugin,
  packageRoot: string,
  packagePath: string,
  evidence: SourceRead[],
  identity: { source: "package"; packageName: string; packageVersion: string },
): Promise<DiscoveredPlugin> {
  const bundlePath = resolve(packageRoot, plugin.bundle);
  const result = localVst3({
    bundlePath,
    classId: plugin.classId,
    ...(plugin.sha256 ? { expectedHash: plugin.sha256 } : {}),
  });
  Object.assign(result.entry, {
    ...identity,
    source: "vst3",
    pluginId: plugin.pluginId,
    pluginVersion: plugin.pluginVersion,
    displayName: plugin.displayName,
    vendor: plugin.vendor,
    handle: sourceHash(`${packagePath}:vst3:${JSON.stringify(plugin)}`),
    kind: plugin.kind,
    ...(plugin.license ? { license: plugin.license } : {}),
  });
  result.reads = evidence;
  try {
    const contained = (path: string) => {
      const suffix = relative(packageRoot, path);
      return !isAbsolute(suffix) && suffix !== ".." && !suffix.startsWith("../");
    };
    if (isAbsolute(plugin.bundle) || !contained(bundlePath))
      throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 bundle escapes its package");
    const info = await lstat(bundlePath);
    if (!info.isDirectory() || !bundlePath.endsWith(".vst3") || !contained(await realpath(bundlePath)))
      throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 bundle must be a package-local directory");
  } catch (error) {
    delete result.vst3Source;
    result.entry.availability = "missing";
    result.entry.diagnostic = String(error);
  }
  return result;
}
