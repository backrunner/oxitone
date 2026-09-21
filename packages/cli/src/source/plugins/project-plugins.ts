import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createEngine, dispose, getPluginInfo } from "@oxitone/native";
import { Vst3Workbench } from "./vst3-workbench.js";
import { localVst3, vst3ParameterSpecs, vst3PluginKind } from "./vst3-discovery.js";
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
  private readonly vst3: Vst3Workbench;
  constructor(private readonly root: string) {
    this.vst3 = new Vst3Workbench(root);
  }
  get dependencyPaths(): string[] {
    return this.reads.flatMap((read) => [read.path, read.realPath]);
  }
  get vst3Bundles(): string[] | undefined {
    return this.vst3.scannedBundles;
  }
  async checkReads(): Promise<void> {
    await checkSourceReads(this.reads);
  }
  async vst3Command(
    command: import("@oxitone/protocol").Vst3WorkbenchCommand,
    frame: PreviewSnapshotFrame | undefined,
    signal: AbortSignal,
    check: () => void,
  ): Promise<void> {
    check();
    if (command.kind === "cancel") return this.vst3.cancel();
    if (command.kind === "scan") return this.vst3.scan(signal, check);
    if (
      command.kind === "controlInstance" ||
      command.kind === "captureInstance" ||
      command.kind === "startRecording" ||
      command.kind === "stopRecording" ||
      command.kind === "cancelRecording"
    )
      throw new OxitoneError(ErrorCode.InvalidProject, "Instance editing requires a source transaction");
    if (command.kind === "addBundle") {
      const plugins = await this.vst3.addBundle(
        command.bundlePath,
        frame?.allowPlugins ?? "signed-only",
        signal,
        check,
      );
      for (const plugin of plugins) {
        const index = this.plugins.findIndex((item) => item.entry.handle === plugin.entry.handle);
        if (index < 0) this.plugins.push(plugin);
        else this.plugins[index] = plugin;
      }
      return;
    }
    if (command.kind === "add" || command.kind === "loadPreset") {
      const plugin =
        command.kind === "add" ? this.vst3.add(command.source) : await this.vst3.loadPreset(command.path, check);
      const index = this.plugins.findIndex((item) => item.entry.handle === plugin.entry.handle);
      if (index < 0) this.plugins.push(plugin);
      else this.plugins[index] = plugin;
      return;
    }
    if (command.kind === "remove") {
      this.vst3.remove(command.plugin);
      this.plugins = this.plugins.filter((plugin) => plugin.entry.handle !== command.plugin);
      return;
    }
    const plugin = this.plugins.find((plugin) => plugin.entry.handle === command.plugin);
    if (!plugin) throw new OxitoneError(ErrorCode.EditTargetMissing, "VST3 selection expired");
    if (command.kind === "edit")
      return this.vst3.edit(plugin, command.parameters, frame?.allowPlugins ?? "signed-only", signal, check);
    if (command.kind === "savePreset") return this.vst3.savePreset(plugin, command.path, command.parameters, check);
    if (command.kind === "attachRender")
      throw new OxitoneError(ErrorCode.InvalidProject, "Attach requires a source transaction");
    await this.vst3.render(plugin, command.options, frame?.allowPlugins ?? "signed-only", signal, check);
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
    const plugins = [...builtins(), ...discovery.plugins, ...this.vst3.refresh()];
    for (const registration of frame?.vst3Plugins ?? []) {
      const id = `vst3.${registration.source.classId.toLowerCase()}`;
      const version = `0.0.0+${registration.metadata.sha256}`;
      if (plugins.some((p) => p.entry.pluginId === id && p.entry.pluginVersion === version)) continue;
      const candidate = localVst3(registration.source);
      const existing = plugins.find((p) => p.entry.handle === candidate.entry.handle);
      const plugin = existing ?? candidate;
      plugin.vst3Info ??= registration.metadata;
      Object.assign(plugin.entry, {
        pluginId: id,
        pluginVersion: version,
        displayName: registration.metadata.name,
        vendor: registration.metadata.vendor,
        kind: vst3PluginKind(registration.metadata),
        parameters: vst3ParameterSpecs(registration.metadata),
      });
      if (!existing) plugins.push(plugin);
    }
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
    if (plugin.entry.vst3) return this.vst3.inspect(plugin, frame?.allowPlugins ?? "signed-only", signal, check);
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
