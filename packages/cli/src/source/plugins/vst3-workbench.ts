import { resolve } from "node:path";
import {
  inspectVst3Plugin,
  configureVst3Plugin,
  editVst3Plugin,
  renderVst3Wav,
  loadVst3Preset,
  saveVst3Preset,
  listVst3Classes,
  scanVst3Bundles,
} from "@oxitone/vst3";
import { ErrorCode, OxitoneError, vst3SourceSchema, type Vst3WorkbenchCommand } from "@oxitone/protocol";
import { checkSourceReads } from "../files/read-set.js";
import type { DiscoveredPlugin } from "./plugin-discovery.js";
import { localVst3 } from "./vst3-discovery.js";
import { publishVst3Inspection } from "./vst3-inspection.js";
import { Vst3Configurations } from "./vst3-configurations.js";

type Host = {
  inspect: typeof inspectVst3Plugin;
  render: typeof renderVst3Wav;
  edit?: typeof editVst3Plugin;
  configure?: typeof configureVst3Plugin;
};
type Policy = "signed-only" | "any";

/** Session-only tools. Configuration is bounded and never broadcast in DocumentView. */
export class Vst3Workbench {
  private readonly locals = new Map<string, DiscoveredPlugin>();
  private readonly states = new Vst3Configurations();
  private active: AbortController | undefined;
  private bundles: string[] | undefined;
  get scannedBundles(): string[] | undefined {
    return this.bundles?.slice();
  }
  async scan(signal: AbortSignal, check: () => void): Promise<void> {
    await this.task(signal, async (taskSignal) => {
      const bundles = await scanVst3Bundles({ signal: taskSignal });
      taskSignal.throwIfAborted();
      check();
      this.bundles = bundles;
    });
  }
  private readonly projectionBytes = new Map<string, number>();
  constructor(
    private readonly root: string,
    private readonly host: Host = {
      inspect: inspectVst3Plugin,
      render: renderVst3Wav,
    },
  ) {}

  refresh(): DiscoveredPlugin[] {
    this.cancel();
    this.states.refresh();
    this.projectionBytes.clear();
    for (const [key, plugin] of this.locals) {
      const refreshed = localVst3(plugin.vst3Source!);
      const path = this.states.path(key);
      if (path) refreshed.entry.vst3!.presetPath = path;
      this.locals.set(key, refreshed);
    }
    return [...this.locals.values()];
  }
  cancel(): void {
    this.active?.abort();
  }
  async addBundle(path: string, policy: Policy, signal: AbortSignal, check: () => void): Promise<DiscoveredPlugin[]> {
    return this.task(signal, async (taskSignal) => {
      const bundlePath = resolve(this.root, path);
      const result = await listVst3Classes({ bundlePath, allowPlugins: policy }, { signal: taskSignal });
      taskSignal.throwIfAborted();
      check();
      const candidates = result.classes.map((c) =>
        localVst3({ bundlePath, classId: c.classId, expectedHash: result.sha256 }),
      );
      if (this.locals.size + candidates.filter((c) => !this.locals.has(c.entry.handle)).length > 32)
        throw new OxitoneError(ErrorCode.BudgetExceeded, "At most 32 local VST3 selections per session");
      return candidates.map((candidate, index) => {
        const plugin = this.add(candidate.vst3Source!);
        Object.assign(plugin.entry, {
          displayName: result.classes[index]!.name,
          vendor: result.vendor,
          pluginVersion: `0.0.0+${result.sha256}`,
        });
        return plugin;
      });
    });
  }
  add(input: Extract<Vst3WorkbenchCommand, { kind: "add" }>["source"]): DiscoveredPlugin {
    const source = vst3SourceSchema.parse({
      ...input,
      bundlePath: resolve(this.root, input.bundlePath),
      classId: input.classId.toLowerCase(),
    });
    const plugin = localVst3(source);
    const previous = this.locals.get(plugin.entry.handle);
    if (previous) return previous;
    if (this.locals.size >= 32)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "At most 32 local VST3 selections per session");
    this.locals.set(plugin.entry.handle, plugin);
    return plugin;
  }
  remove(handle: string): void {
    if (!this.locals.delete(handle))
      throw new OxitoneError(ErrorCode.EditScopeConflict, "Only session-local VST3 entries can be removed");
    this.states.remove(handle);
    this.projectionBytes.delete(handle);
  }
  private async task<T>(signal: AbortSignal, run: (signal: AbortSignal) => Promise<T>): Promise<T> {
    if (this.active) throw new OxitoneError(ErrorCode.PluginTaskConflict, "A VST3 task is already running");
    const controller = new AbortController();
    this.active = controller;
    try {
      return await run(AbortSignal.any([signal, controller.signal]));
    } finally {
      if (this.active === controller) this.active = undefined;
    }
  }
  async inspect(plugin: DiscoveredPlugin, policy: Policy, signal: AbortSignal, check: () => void): Promise<void> {
    const source = plugin.vst3Source;
    if (!source) throw new OxitoneError(ErrorCode.AssetUnavailable, "VST3 bundle is unavailable");
    this.states.forgetInspection(plugin.entry.handle);
    this.projectionBytes.delete(plugin.entry.handle);
    plugin.entry.validation = "unverified";
    plugin.entry.vst3!.parameters = [];
    delete plugin.entry.vst3!.render;
    try {
      await this.task(signal, async (taskSignal) => {
        await checkSourceReads(plugin.reads);
        check();
        const retained = this.states.retained(plugin.entry.handle);
        const selected = { ...source, allowPlugins: policy };
        const info = retained
          ? await (this.host.configure ?? configureVst3Plugin)(
              selected,
              { configuration: retained },
              { signal: taskSignal },
            )
          : await this.host.inspect(selected, { signal: taskSignal });
        await checkSourceReads(plugin.reads);
        taskSignal.throwIfAborted();
        check();
        if (info.classId.toLowerCase() !== source.classId.toLowerCase())
          throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 class changed during inspection");
        publishVst3Inspection(plugin, info, this.states, this.projectionBytes);
      });
    } catch (error) {
      check();
      plugin.entry.validation = "failed";
      plugin.entry.diagnostic = String(error);
      throw error;
    }
  }
  async loadPreset(path: string, check: () => void): Promise<DiscoveredPlugin> {
    path = resolve(this.root, path);
    const preset = await loadVst3Preset(path);
    check();
    const source = { ...preset.source, expectedHash: preset.configuration.sha256 };
    const candidate = localVst3(source);
    if (!this.locals.has(candidate.entry.handle) && this.locals.size >= 32)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "At most 32 local VST3 selections per session");
    this.states.load(candidate.entry.handle, preset, path);
    candidate.entry.displayName = preset.name;
    candidate.entry.vst3!.presetPath = path;
    this.locals.set(candidate.entry.handle, candidate);
    this.projectionBytes.delete(candidate.entry.handle);
    return candidate;
  }
  async savePreset(
    plugin: DiscoveredPlugin,
    path: string,
    parameters: Record<string, number>,
    check: () => void,
  ): Promise<void> {
    if (plugin.entry.validation !== "verified" || !plugin.vst3Source)
      throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Inspect this VST3 plugin before saving a preset");
    const configuration = this.states.get(plugin.entry.handle);
    if (!configuration) throw new OxitoneError(ErrorCode.PluginConfigInvalid, "VST3 configuration was not captured");
    for (const id of Object.keys(parameters)) {
      if (!plugin.entry.vst3?.parameters.some((p) => p.id.toString() === id && !p.readOnly))
        throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Unknown or read-only VST3 parameter");
    }
    await checkSourceReads(plugin.reads);
    check();
    await saveVst3Preset(resolve(this.root, path), {
      formatVersion: 1,
      kind: "oxitone-vst3-preset",
      name: plugin.entry.displayName,
      source: { bundlePath: plugin.vst3Source.bundlePath, classId: plugin.vst3Source.classId },
      configuration: { ...configuration, parameters: { ...configuration.parameters, ...parameters } },
    });
    check();
    plugin.entry.vst3!.presetPath = resolve(this.root, path);
  }
  async edit(
    plugin: DiscoveredPlugin,
    parameters: Record<string, number>,
    policy: Policy,
    signal: AbortSignal,
    check: () => void,
  ): Promise<void> {
    if (!plugin.vst3Source || plugin.entry.validation !== "verified" || !plugin.entry.sha256)
      throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Inspect this VST3 plugin before opening its editor");
    await this.task(signal, async (taskSignal) => {
      await checkSourceReads(plugin.reads);
      check();
      const info = await (this.host.edit ?? editVst3Plugin)(
        { ...plugin.vst3Source!, expectedHash: plugin.entry.sha256!, allowPlugins: policy },
        { configuration: this.states.get(plugin.entry.handle), parameters },
        { signal: taskSignal },
      );
      taskSignal.throwIfAborted();
      await checkSourceReads(plugin.reads);
      check();
      if (!info) return;
      if (
        !info.configuration ||
        info.sha256 !== plugin.entry.sha256 ||
        info.classId.toLowerCase() !== plugin.vst3Source!.classId.toLowerCase()
      )
        throw new OxitoneError(ErrorCode.PluginManifestMismatch, "VST3 editor identity changed");
      publishVst3Inspection(plugin, info, this.states, this.projectionBytes, "editor");
    });
  }
  async render(
    plugin: DiscoveredPlugin,
    options: Extract<Vst3WorkbenchCommand, { kind: "render" }>["options"],
    policy: Policy,
    signal: AbortSignal,
    check: () => void,
  ): Promise<void> {
    if (!plugin.vst3Source || !plugin.entry.vst3 || plugin.entry.validation !== "verified" || !plugin.entry.sha256)
      throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Inspect this VST3 plugin before rendering");
    const parameters = new Map(plugin.entry.vst3.parameters.map((parameter) => [parameter.id, parameter]));
    for (const id of Object.keys(options.parameters)) {
      if (!parameters.has(Number(id)) || parameters.get(Number(id))!.readOnly)
        throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Unknown or read-only VST3 parameter");
    }
    for (const event of options.events) {
      if (event.type === "parameter") {
        const parameter = parameters.get(event.parameterId);
        if (!parameter || parameter.readOnly || !parameter.canAutomate)
          throw new OxitoneError(ErrorCode.PluginConfigInvalid, "VST3 parameter cannot be automated");
      } else if (!plugin.entry.vst3.noteInput)
        throw new OxitoneError(ErrorCode.PluginCapabilityUnsupported, "VST3 plugin has no note input");
    }
    delete plugin.entry.vst3.render;
    await this.task(signal, async (taskSignal) => {
      await checkSourceReads(plugin.reads);
      check();
      const configuration = this.states.get(plugin.entry.handle);
      const report = await this.host.render(
        {
          ...plugin.vst3Source!,
          expectedHash: plugin.entry.sha256!,
          allowPlugins: policy,
        },
        {
          ...options,
          path: resolve(this.root, options.path),
          ...(options.inputPath ? { inputPath: resolve(this.root, options.inputPath) } : {}),
          ...(configuration ? { configuration } : {}),
        },
        { signal: taskSignal },
      );
      taskSignal.throwIfAborted();
      await checkSourceReads(plugin.reads);
      check();
      plugin.entry.vst3!.render = report;
    });
  }
}
