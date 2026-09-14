import {
  ErrorCode,
  OxitoneError,
  frameToWire,
  type EngineDiagnostics,
  type EntityId,
  type MidiExportOptions,
  type MidiExportReport,
  type OutputLatency,
  type ProjectSnapshot,
  type CompileOptions,
  type RenderOptions,
  type RenderReport,
  type TransportCommand,
  type TransportState,
  type EngineOptions,
  type RegisterPluginOptions,
  type RegisteredPlugin,
  type PluginDiagnostics,
} from "@oxitone/protocol";
import {
  createEngine,
  compile as nativeCompile,
  dispose as nativeDispose,
  enqueueTransport,
  exportMidi as nativeExportMidi,
  getDiagnostics as nativeGetDiagnostics,
  getOutputLatency as nativeGetOutputLatency,
  renderWav as nativeRenderWav,
  setParameter as nativeSetParameter,
  type EngineHandle,
  registerPlugin as nativeRegisterPlugin,
  getPluginDiagnostics,
} from "@oxitone/native";
import { positionFields, type TransportPosition } from "../timing/transport-position.js";
export type { TransportPosition } from "../timing/transport-position.js";

/** Absolute sample-frame loop region. End is exclusive. */
export type LoopRegion = { startFrame: bigint | number; endFrame: bigint | number };

/**
 * Live engine session holding an `engineId` (04-api-contracts.md §Facade 与
 * commands). Created by `Project.compile()` / `Project.play()`; transport
 * and `setParameter` act on the engine's compiled graph. The first `play()`
 * starts realtime output on the configured CoreAudio device; before that
 * the transport advances offline state only (used by `renderWav`).
 */
export class Session {
  private disposedValue = false;
  /** @internal Use `Project.compile()` instead. */
  constructor(
    private readonly engine: EngineHandle,
    private readonly snapshotProvider: () => ProjectSnapshot,
    private compiledSnapshot: ProjectSnapshot,
    private readonly compileOptions: CompileOptions = {},
  ) {}

  get engineId(): string {
    return this.engine.id;
  }

  get disposed(): boolean {
    return this.disposedValue;
  }
  get revision(): bigint {
    return BigInt(this.compiledSnapshot.revision);
  }

  registerPlugin(options: RegisterPluginOptions): RegisteredPlugin {
    this.assertActive();
    return nativeRegisterPlugin(this.engine, options);
  }

  pluginDiagnostics(): PluginDiagnostics[] {
    this.assertActive();
    return getPluginDiagnostics(this.engine);
  }

  private assertActive(): void {
    if (this.disposedValue) throw new OxitoneError(ErrorCode.InvalidProject, "session has been disposed");
  }

  /** Compile the current authoring snapshot on the same engine; rejection preserves the previous graph. */
  async update(): Promise<this> {
    this.assertActive();
    const snapshot = nativeCompile(this.engine, this.snapshotProvider(), this.compileOptions);
    this.compiledSnapshot = snapshot;
    return this;
  }

  private async transport(
    command: TransportCommand["command"],
    position?: TransportPosition,
    loop?: LoopRegion,
  ): Promise<TransportState> {
    this.assertActive();
    const loopRegion =
      loop === undefined
        ? undefined
        : {
            startFrame: frameToWire(loop.startFrame),
            endFrame: frameToWire(loop.endFrame),
          };
    return enqueueTransport(this.engine, { command, ...positionFields(position, this.compiledSnapshot), loopRegion });
  }

  /** Start playback (optionally from a position). The first call opens the
   * output device and starts realtime audio. */
  async play(position?: TransportPosition, loop?: LoopRegion): Promise<TransportState> {
    return this.transport("play", position, loop);
  }

  async pause(): Promise<TransportState> {
    return this.transport("pause");
  }

  async stop(): Promise<TransportState> {
    return this.transport("stop");
  }

  /** Move the transport cursor (voices/DSP state are flushed). */
  async seek(position: TransportPosition): Promise<TransportState> {
    return this.transport("seek", position);
  }

  /**
   * Ring horizon + device output latency breakdown (project-rate frames +
   * seconds). Requires an active realtime session (call `play()` first).
   */
  async outputLatency(): Promise<OutputLatency> {
    this.assertActive();
    return nativeGetOutputLatency(this.engine);
  }

  /**
   * Realtime counters, `engineLoad` EMA, ring occupancy, block-time
   * percentiles, and the diagnostic events raised since the last call.
   * Requires an active realtime session.
   */
  async diagnostics(): Promise<EngineDiagnostics> {
    this.assertActive();
    return nativeGetDiagnostics(this.engine);
  }

  /**
   * Queue a physical-value parameter event; validated against the compiled
   * graph and applied to subsequent offline renders.
   */
  async setParameter(entityId: EntityId, parameterId: string, value: number, atFrame?: bigint | number): Promise<void> {
    this.assertActive();
    nativeSetParameter(this.engine, entityId, parameterId, value, atFrame);
  }

  /** Offline WAV export of the last successfully compiled snapshot. */
  async renderWav(options: RenderOptions): Promise<RenderReport> {
    this.assertActive();
    return nativeRenderWav(this.engine, this.compiledSnapshot, {
      assetBaseDir: this.compileOptions.assetBaseDir,
      ...options,
    });
  }

  /** SMF Type 1 export of the last successfully compiled snapshot. */
  async exportMidi(options: MidiExportOptions): Promise<MidiExportReport> {
    this.assertActive();
    return nativeExportMidi(this.engine, this.compiledSnapshot, options);
  }

  /** Release the native engine. Further calls fail with `InvalidProject`. */
  async dispose(): Promise<void> {
    if (this.disposedValue) return;
    nativeDispose(this.engine);
    this.disposedValue = true;
  }
}

/** @internal Create a throwaway engine around one snapshot call. */
export async function withTempEngine<T>(
  run: (engine: EngineHandle) => T,
  options?: EngineOptions,
  plugins: readonly RegisterPluginOptions[] = [],
): Promise<T> {
  const engine = createEngine(options);
  try {
    for (const plugin of plugins) nativeRegisterPlugin(engine, plugin);
    return run(engine);
  } finally {
    nativeDispose(engine);
  }
}
