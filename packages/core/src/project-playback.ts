import {
  ErrorCode,
  OxitoneError,
  type EngineOptions,
  type CompileOptions,
  type MidiExportOptions,
  type MidiExportReport,
  type ProjectSnapshot,
  type RenderOptions,
  type RenderReport,
  registerPluginOptionsSchema,
  type RegisterPluginOptions,
  type RegisteredPlugin,
} from "@oxitone/protocol";
import {
  compile as nativeCompile,
  createEngine,
  dispose as nativeDispose,
  renderWav as nativeRenderWav,
  exportMidi as nativeExportMidi,
  registerPlugin as nativeRegisterPlugin,
} from "@oxitone/native";
import { resolve } from "node:path";
import { Session, withTempEngine, type TransportPosition, type LoopRegion } from "./session.js";
import type { BarBeatPosition } from "./time-signature.js";

/** Native session lifecycle, separate from project authoring state. */
export type ProjectCompileOptions = EngineOptions & CompileOptions;

export abstract class ProjectPlayback {
  private activeSession?: Session;
  private pluginList: RegisterPluginOptions[] = [];
  private policy: EngineOptions["allowPlugins"];
  get registeredPlugins(): RegisterPluginOptions[] { return structuredClone(this.pluginList); }
  get pluginPolicy(): EngineOptions["allowPlugins"] { return this.policy; }

  /** Register trusted plugin code for this project's current and future engines. */
  registerPlugin(input: RegisterPluginOptions, options: Pick<EngineOptions, "allowPlugins"> = {}): RegisteredPlugin {
    const plugin = registerPluginOptionsSchema.parse(input);
    plugin.libraryPath = resolve(plugin.libraryPath);
    const policy = options.allowPlugins ?? this.policy;
    const engine = createEngine({ allowPlugins: policy });
    try {
      for (const existing of this.pluginList) nativeRegisterPlugin(engine, existing);
      const result = nativeRegisterPlugin(engine, plugin);
      this.session?.registerPlugin({ ...plugin, expectedHash: result.sha256 });
      if (!this.pluginList.some((entry) => entry.manifest.pluginId === result.pluginId && entry.manifest.pluginVersion === result.pluginVersion)) {
        this.pluginList.push({ ...plugin, expectedHash: result.sha256 });
      }
      this.policy = policy;
      return result;
    } finally { nativeDispose(engine); }
  }
  protected projectAssetBaseDir: string | undefined;
  /** Absolute resource directory retained when restoring a portable project. */
  get assetBaseDir(): string | undefined { return this.projectAssetBaseDir; }

  abstract snapshot(): ProjectSnapshot;
  abstract barBeatToBeats(position: BarBeatPosition): number;

  /**
   * Create a native engine and compile the current snapshot; the returned
   * session holds the `engineId` for transport, `setParameter`, offline
   * render and MIDI export. Replaces (and disposes) any previous session.
   */
  async compile(options?: ProjectCompileOptions): Promise<Session> {
    options = { assetBaseDir: this.assetBaseDir, allowPlugins: this.policy, ...options };
    const engine = createEngine(options);
    let snapshot: ProjectSnapshot;
    try {
      for (const plugin of this.pluginList) nativeRegisterPlugin(engine, plugin);
      snapshot = nativeCompile(engine, this.snapshot(), { assetBaseDir: options?.assetBaseDir });
    } catch (error) {
      nativeDispose(engine);
      throw error;
    }
    const previous = this.activeSession;
    this.activeSession = new Session(engine, () => this.snapshot(), snapshot, { assetBaseDir: options?.assetBaseDir });
    if (previous !== undefined) {
      await previous.dispose();
    }
    return this.activeSession;
  }

  /** The session from the last `compile()`/`play()`, if still active. */
  get session(): Session | undefined {
    return this.activeSession?.disposed ? undefined : this.activeSession;
  }

  private requireSession(): Session {
    const session = this.session;
    if (session === undefined) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "no active session; call compile() or play() first",
      );
    }
    return session;
  }

  /** One-shot offline WAV export on a temporary engine (04 §RenderOptions). */
  async renderWav(options: RenderOptions): Promise<RenderReport> {
    const snapshot = this.snapshot();
    return withTempEngine((engine) => nativeRenderWav(engine, snapshot, { assetBaseDir: this.assetBaseDir, ...options }),
      { allowPlugins: this.policy }, this.pluginList);
  }

  /** One-shot SMF Type 1 export on a temporary engine. */
  async exportMidi(options: MidiExportOptions): Promise<MidiExportReport> {
    const snapshot = this.snapshot();
    return withTempEngine((engine) => nativeExportMidi(engine, snapshot, options));
  }

  /** Compile/update the authored revision and start playback; returns the active session. */
  async play(position?: TransportPosition, loop?: LoopRegion): Promise<Session> {
    const session = this.session ?? (await this.compile());
    if (session.revision !== BigInt(this.snapshot().revision)) await session.update();
    await session.play(position, loop);
    return session;
  }

  async pause(): Promise<void> {
    await this.requireSession().pause();
  }

  async stop(): Promise<void> {
    await this.requireSession().stop();
  }

  async seek(position: TransportPosition): Promise<void> {
    await this.requireSession().seek(position);
  }
}
