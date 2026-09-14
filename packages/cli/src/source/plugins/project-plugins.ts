import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createEngine, dispose, getPluginInfo } from "@oxitone/native";
import {
  effectPluginIds,
  engineOptionsSchema,
  ErrorCode,
  OxitoneError,
  type PluginCatalogEntry,
  type PreviewSnapshotFrame,
  type PluginInfo,
  type RegisteredPlugin,
} from "@oxitone/protocol";
import { discoverProjectPlugins, type DiscoveredPlugin } from "./plugin-discovery.js";
import { captureSourceReads, checkSourceReads, type SourceRead } from "../files/read-set.js";
import { runEvaluationProcess } from "../eval/evaluation-process.js";
import { sourceHash } from "../syntax/program.js";

function builtins(): DiscoveredPlugin[] {
  if (builtinCache) return structuredClone(builtinCache);
  const engine = createEngine(engineOptionsSchema.parse({}));
  try {
    builtinCache = [
      "oxitone.wavetable",
      "oxitone.sampler",
      "oxitone.multisampler",
      "oxitone.slicer",
      ...Object.values(effectPluginIds),
    ].map((id) => {
      const info = getPluginInfo(engine, id, "1.0.0");
      return {
        reads: [],
        entry: {
          handle: sourceHash(id),
          pluginId: id,
          pluginVersion: "1.0.0",
          displayName: id.slice(8),
          vendor: "Oxitone",
          source: "builtin",
          kind: info.kind,
          availability: "available",
          validation: "verified",
          parameters: info.parameters,
          usages: [],
        },
      };
    });
    return structuredClone(builtinCache);
  } finally {
    dispose(engine);
  }
}
let builtinCache: DiscoveredPlugin[] | undefined;

/** Catalog/verification tasks are control work; browsing never loads an external library. */
export class ProjectPluginCatalog {
  private plugins = builtins();
  private reads: SourceRead[] = [];
  constructor(private readonly root: string) {}
  get dependencyPaths(): string[] {
    return this.reads.flatMap((read) => [read.path, read.realPath]);
  }
  async checkReads(): Promise<void> {
    await checkSourceReads(this.reads);
  }
  selection(handle: string): DiscoveredPlugin {
    const plugin = this.plugins.find((p) => p.entry.handle === handle);
    if (!plugin || plugin.entry.availability !== "available" || plugin.entry.validation !== "verified") {
      throw new OxitoneError(ErrorCode.EditTargetMissing, "Choose an available, verified plugin");
    }
    return structuredClone({ ...plugin, reads: [...this.reads, ...plugin.reads] });
  }
  async refresh(frame: PreviewSnapshotFrame | undefined, check: () => void): Promise<void> {
    const discovery = await discoverProjectPlugins(this.root);
    check();
    const plugins = [...builtins(), ...discovery.plugins];
    for (const registration of frame?.plugins ?? []) {
      if (
        plugins.some(
          (plugin) =>
            plugin.registration?.libraryPath === registration.libraryPath &&
            plugin.entry.pluginId === registration.manifest.pluginId &&
            plugin.entry.pluginVersion === registration.manifest.pluginVersion,
        )
      )
        continue;
      const manifest = registration.manifest;
      plugins.push({
        registration,
        reads: [],
        entry: {
          handle: sourceHash(JSON.stringify(registration)),
          pluginId: manifest.pluginId,
          pluginVersion: manifest.pluginVersion,
          displayName: manifest.pluginId,
          vendor: "",
          source: "project",
          kind: manifest.kind,
          availability: "available",
          validation: "unverified",
          libraryPath: registration.libraryPath,
          ...(registration.expectedHash ? { sha256: registration.expectedHash } : {}),
          parameters: manifest.parameters,
          usages: [],
        },
      });
    }
    await checkSourceReads(discovery.reads);
    check();
    this.plugins = plugins;
    this.reads = discovery.reads;
  }
  view(frame?: PreviewSnapshotFrame): PluginCatalogEntry[] {
    const entries = structuredClone(this.plugins.map((plugin) => plugin.entry));
    const add = (pluginId: string, pluginVersion: string, usage: PluginCatalogEntry["usages"][number]) => {
      let matching = entries.filter((entry) => entry.pluginId === pluginId && entry.pluginVersion === pluginVersion);
      if (!matching.length) {
        const entry: PluginCatalogEntry = {
          handle: sourceHash(`missing:${pluginId}@${pluginVersion}`),
          pluginId,
          pluginVersion,
          displayName: pluginId,
          vendor: "",
          source: "project",
          kind: usage.kind === "instrument" ? "instrument" : "effect",
          availability: "missing",
          validation: "unverified",
          parameters: [],
          usages: [],
        };
        entries.push(entry);
        matching = [entry];
      }
      for (const entry of matching) entry.usages.push(usage);
    };
    for (const channel of frame?.snapshot.channels ?? []) {
      add(channel.instrument.pluginId, channel.instrument.pluginVersion, {
        kind: "instrument",
        owner: channel.id,
        instanceId: channel.instrument.instanceId,
        index: 0,
        label: channel.name ?? channel.id,
      });
      channel.effectChain?.forEach((effect, index) =>
        add(effect.pluginId, effect.pluginVersion, {
          kind: "channelInsert",
          owner: channel.id,
          instanceId: effect.instanceId,
          index,
          label: channel.name ?? channel.id,
        }),
      );
    }
    for (const bus of frame?.snapshot.mixerChannels ?? [])
      bus.inserts?.forEach((effect, index) =>
        add(effect.pluginId, effect.pluginVersion, {
          kind: "busInsert",
          owner: bus.id,
          instanceId: effect.instanceId,
          index,
          label: bus.name ?? bus.id,
        }),
      );
    return entries;
  }
  async verify(
    handle: string,
    frame: PreviewSnapshotFrame | undefined,
    signal: AbortSignal,
    check: () => void,
  ): Promise<void> {
    const plugin = this.plugins.find((plugin) => plugin.entry.handle === handle);
    if (!plugin) throw new OxitoneError(ErrorCode.EditTargetMissing, "plugin catalog selection expired");
    if (plugin.entry.source === "builtin") return;
    if (!plugin.registration)
      throw new OxitoneError(ErrorCode.AssetUnavailable, "plugin has no available platform library");
    const directory = await mkdtemp(join(tmpdir(), "oxitone-plugin-verify-"));
    try {
      await checkSourceReads(plugin.reads);
      check();
      const libraryReads = await captureSourceReads([plugin.registration.libraryPath]);
      const input = join(directory, "verify.json");
      await writeFile(
        input,
        JSON.stringify({ registration: plugin.registration, allowPlugins: frame?.allowPlugins ?? "signed-only" }),
      );
      const result = (await runEvaluationProcess(input, "", this.root, signal, 10_000, "plugin-worker")) as {
        error?: { code: import("@oxitone/protocol").OxitoneErrorCode; message: string };
        info: PluginInfo;
        registered: RegisteredPlugin;
      };
      check();
      await checkSourceReads([...plugin.reads, ...libraryReads]);
      check();
      if (result.error) throw new OxitoneError(result.error.code, result.error.message);
      plugin.entry.parameters = result.info.parameters;
      plugin.entry.sha256 = result.registered.sha256;
      plugin.entry.validation = "verified";
      delete plugin.entry.diagnostic;
      this.reads.push(...libraryReads);
    } catch (error) {
      check();
      plugin.entry.validation = "failed";
      plugin.entry.diagnostic = String(error);
      throw error;
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  }
}
