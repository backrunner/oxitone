import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import type * as generated from "@oxitone/native-generated";

export interface NativeBinding {
  resolveBeatDuration: typeof generated.resolveBeatDuration;
  inspectSample: typeof generated.inspectSample;
  cacheSample: typeof generated.cacheSample;
  createEngine: typeof generated.createEngine;
  compile: typeof generated.compile;
  registerPlugin: typeof generated.registerPlugin;
  getPluginDiagnostics: typeof generated.getPluginDiagnostics;
  getPluginInfo: typeof generated.getPluginInfo;
  dispose: typeof generated.dispose;
  enqueueTransport: typeof generated.enqueueTransport;
  exportMidi: typeof generated.exportMidi;
  getDiagnostics: typeof generated.getDiagnostics;
  getOutputLatency: typeof generated.getOutputLatency;
  getProtocolVersion: typeof generated.getProtocolVersion;
  listOutputDevices: typeof generated.listOutputDevices;
  renderWav: typeof generated.renderWav;
  setParameter: typeof generated.setParameter;
}

const require = createRequire(import.meta.url);
const repoRoot = fileURLToPath(new URL("../../..", import.meta.url));
const binaryStem = `oxitone-native.${process.platform}-${process.arch}`;

function siblingBinary(resolveTarget: string): string | undefined {
  try {
    const packageJson = require.resolve(resolveTarget);
    const candidate = join(dirname(packageJson), `${binaryStem}.node`);
    return existsSync(candidate) ? candidate : undefined;
  } catch {
    return undefined;
  }
}

function nativeGeneratedBinary(): string | undefined {
  return siblingBinary("@oxitone/native-generated/package.json");
}

function platformPackageBinary(): string | undefined {
  // Optional platform packages use the reserved
  // `@oxitone/native-<platform>-<arch>` name; local build outputs remain a
  // development fallback until those packages are published.
  return siblingBinary(
    `@oxitone/native-${process.platform}-${process.arch}/package.json`,
  );
}

function devBuildBinaries(): string[] {
  return [
    join(repoRoot, "crates", "napi", `${binaryStem}.node`),
    join(repoRoot, "target", "release", "liboxitone_napi.dylib"),
    join(repoRoot, "target", "debug", "liboxitone_napi.dylib"),
  ];
}

function loadAddon(path: string): NativeBinding {
  if (path.endsWith(".node")) {
    return require(path) as NativeBinding;
  }
  const addon = { exports: {} as Record<string, unknown> };
  process.dlopen(addon, path);
  return addon.exports as unknown as NativeBinding;
}

export function loadNativeBinding(): NativeBinding {
  const candidates = [
    process.env.OXITONE_NATIVE_PATH,
    nativeGeneratedBinary(),
    platformPackageBinary(),
    ...devBuildBinaries(),
  ];
  const attempted: string[] = [];
  for (const candidate of candidates) {
    if (candidate === undefined) {
      continue;
    }
    attempted.push(candidate);
    if (!existsSync(candidate)) {
      continue;
    }
    try {
      return loadAddon(candidate);
    } catch {
      continue;
    }
  }
  throw new OxitoneError(
    ErrorCode.AssetUnavailable,
    `oxitone native binding not found; run \`pnpm build:native\` first (searched: ${attempted.join(", ") || "<none>"})`,
  );
}
