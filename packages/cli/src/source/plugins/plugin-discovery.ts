import { readFile, realpath, stat } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import {
  pluginInstallManifestSchema,
  ErrorCode,
  OxitoneError,
  type PluginCatalogEntry,
  type RegisterPluginOptions,
} from "@oxitone/protocol";
import { captureSourceReads, type SourceRead } from "../files/read-set.js";
import { sourceHash } from "../syntax/program.js";

export interface DiscoveredPlugin {
  entry: PluginCatalogEntry;
  registration?: RegisterPluginOptions;
  sourceLibraryPath?: string;
  reads: SourceRead[];
}
const contained = (root: string, path: string) => {
  const suffix = relative(root, path);
  return !isAbsolute(suffix) && suffix !== ".." && !suffix.startsWith("../");
};
export const npmPackageName = /^(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*$/;
async function json(path: string): Promise<{ value: unknown; reads: SourceRead[] }> {
  const info = await stat(path);
  if (!info.isFile() || info.size > 1024 * 1024)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "plugin metadata exceeds 1 MiB");
  const reads = await captureSourceReads([path]);
  const text = await readFile(path, "utf8");
  if (sourceHash(text) !== reads[0]!.sha256)
    throw new OxitoneError(ErrorCode.SourceChanged, "plugin metadata changed while reading");
  return { value: JSON.parse(text), reads };
}
async function metadataPath(root: string, path: string): Promise<string> {
  const candidate = resolve(root, path);
  if (isAbsolute(path) || !contained(root, candidate) || !contained(root, await realpath(candidate))) {
    throw new OxitoneError(ErrorCode.PluginManifestMismatch, "plugin metadata path escapes its package");
  }
  return candidate;
}

/** Only declared dependencies with static oxitone.plugins metadata. Never import JS or load libraries. */
export async function discoverProjectPlugins(
  root: string,
): Promise<{ plugins: DiscoveredPlugin[]; reads: SourceRead[] }> {
  const projectPath = join(root, "package.json");
  let project: { dependencies?: object; devDependencies?: object; optionalDependencies?: object };
  let reads: SourceRead[];
  try {
    const input = await json(projectPath);
    project = input.value as typeof project;
    reads = input.reads;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return { plugins: [], reads: [] };
    throw error;
  }
  const names = Object.keys({
    ...project.dependencies,
    ...project.devDependencies,
    ...project.optionalDependencies,
  }).sort();
  if (names.length > 4096)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "plugin discovery dependency budget exceeded");
  const plugins: DiscoveredPlugin[] = [];
  for (const name of names) {
    if (!npmPackageName.test(name)) continue;
    let directory = root,
      packagePath: string | undefined;
    while (true) {
      const candidate = join(directory, "node_modules", name, "package.json");
      try {
        await stat(candidate);
        packagePath = candidate;
        break;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      }
      const parent = dirname(directory);
      if (parent === directory) break;
      directory = parent;
    }
    if (!packagePath) continue; // An absent arbitrary npm dependency is not necessarily an audio plugin.
    const input = await json(packagePath);
    const metadata = input.value as { version?: string; oxitone?: { plugins?: unknown } };
    if (metadata.oxitone?.plugins === undefined) continue;
    const packageReads = input.reads;
    reads.push(...packageReads);
    const identity = {
      source: "package" as const,
      packageName: name,
      packageVersion: String(metadata.version ?? "unknown"),
    };
    try {
      if (typeof metadata.oxitone.plugins !== "string")
        throw new Error("oxitone.plugins must name a static manifest file");
      const packageRoot = await realpath(dirname(packagePath));
      const installPath = await metadataPath(packageRoot, metadata.oxitone.plugins);
      const installed = await json(installPath);
      const evidence = [...packageReads, ...installed.reads];
      reads.push(...installed.reads);
      const install = pluginInstallManifestSchema.parse(installed.value);
      for (const plugin of install.plugins) {
        const platform = plugin.platforms[`${process.platform}-${process.arch}`];
        const entry: PluginCatalogEntry = {
          ...identity,
          handle: sourceHash(
            `${packagePath}:${plugin.manifest.pluginId}:${plugin.manifest.pluginVersion}:${JSON.stringify(plugin)}`,
          ),
          pluginId: plugin.manifest.pluginId,
          pluginVersion: plugin.manifest.pluginVersion,
          displayName: plugin.displayName,
          vendor: plugin.vendor,
          kind: plugin.manifest.kind,
          availability: platform ? "available" : "unsupported",
          validation: "unverified",
          parameters: plugin.manifest.parameters,
          usages: [],
          ...(plugin.license ? { license: plugin.license } : {}),
        };
        if (!platform) {
          plugins.push({ entry, reads: evidence });
          continue;
        }
        try {
          const libraryPath = await metadataPath(packageRoot, platform.library);
          if (!(await stat(libraryPath)).isFile()) throw new Error("plugin library is not a regular file");
          entry.libraryPath = libraryPath;
          entry.sha256 = platform.sha256;
          plugins.push({
            entry,
            reads: evidence,
            sourceLibraryPath: resolve(dirname(packagePath), platform.library),
            registration: { libraryPath, expectedHash: platform.sha256, manifest: plugin.manifest },
          });
        } catch (error) {
          entry.availability = "missing";
          entry.diagnostic = String(error);
          plugins.push({ entry, reads: evidence });
        }
      }
    } catch (error) {
      plugins.push({
        reads: packageReads,
        entry: {
          ...identity,
          handle: sourceHash(packagePath),
          pluginId: name,
          pluginVersion: identity.packageVersion,
          displayName: name,
          vendor: "",
          kind: "unknown",
          availability: "invalid",
          validation: "unverified",
          diagnostic: String(error),
          parameters: [],
          usages: [],
        },
      });
    }
    if (plugins.length > 4096) throw new OxitoneError(ErrorCode.BudgetExceeded, "plugin catalog exceeds 4096 entries");
  }
  return { plugins, reads };
}
