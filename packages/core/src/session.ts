import {
  beatToWire,
  frameToWire,
  type EngineDiagnostics,
  type EntityId,
  type MidiExportOptions,
  type MidiExportReport,
  type OutputLatency,
  type ProjectSnapshot,
  type RenderOptions,
  type RenderReport,
  type TransportCommand,
  type TransportState,
} from "@oxitone/protocol";
import {
  createEngine,
  dispose as nativeDispose,
  enqueueTransport,
  exportMidi as nativeExportMidi,
  getDiagnostics as nativeGetDiagnostics,
  getOutputLatency as nativeGetOutputLatency,
  renderWav as nativeRenderWav,
  setParameter as nativeSetParameter,
  type EngineHandle,
} from "oxitone";

/** Transport position: a bar (1-based), an absolute beat, or a sample frame. */
export type TransportPosition = { bar: number } | { beat: number } | { frame: bigint | number };

/**
 * Live engine session holding an `engineId` (04-api-contracts.md §Facade 与
 * commands). Created by `Project.compile()` / `Project.play()`; transport
 * and `setParameter` act on the engine's compiled graph. The first `play()`
 * starts realtime output on the configured CoreAudio device; before that
 * the transport advances offline state only (used by `renderWav`).
 */
export class Session {
  /** @internal Use `Project.compile()` instead. */
  constructor(
    private readonly engine: EngineHandle,
    private readonly snapshotProvider: () => ProjectSnapshot,
    private readonly barToBeats: (bar: number) => number,
  ) {}

  get engineId(): string {
    return this.engine.id;
  }

  private positionFields(position?: TransportPosition): Pick<TransportCommand, "frame" | "beat"> {
    if (position === undefined) {
      return {};
    }
    if ("frame" in position) {
      return { frame: frameToWire(position.frame) };
    }
    if ("beat" in position) {
      return { beat: beatToWire(position.beat) };
    }
    return { beat: beatToWire(this.barToBeats(position.bar)) };
  }

  private async transport(
    command: TransportCommand["command"],
    position?: TransportPosition,
  ): Promise<TransportState> {
    return enqueueTransport(this.engine, { command, ...this.positionFields(position) });
  }

  /** Start playback (optionally from a position). The first call opens the
   * output device and starts realtime audio. */
  async play(position?: TransportPosition): Promise<TransportState> {
    return this.transport("play", position);
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
    return nativeGetOutputLatency(this.engine);
  }

  /**
   * Realtime counters, `engineLoad` EMA, ring occupancy, block-time
   * percentiles, and the diagnostic events raised since the last call.
   * Requires an active realtime session.
   */
  async diagnostics(): Promise<EngineDiagnostics> {
    return nativeGetDiagnostics(this.engine);
  }

  /**
   * Queue a physical-value parameter event; validated against the compiled
   * graph and applied to subsequent offline renders.
   */
  async setParameter(
    entityId: EntityId,
    parameterId: string,
    value: number,
    atFrame?: bigint | number,
  ): Promise<void> {
    nativeSetParameter(this.engine, entityId, parameterId, value, atFrame);
  }

  /** Offline WAV export of the current project snapshot. */
  async renderWav(options: RenderOptions): Promise<RenderReport> {
    return nativeRenderWav(this.engine, this.snapshotProvider(), options);
  }

  /** SMF Type 1 export of the current project snapshot. */
  async exportMidi(options: MidiExportOptions): Promise<MidiExportReport> {
    return nativeExportMidi(this.engine, this.snapshotProvider(), options);
  }

  /** Release the native engine. Further calls fail with `InvalidProject`. */
  async dispose(): Promise<void> {
    nativeDispose(this.engine);
  }
}

/** @internal Create a throwaway engine around one snapshot call. */
export async function withTempEngine<T>(run: (engine: EngineHandle) => T): Promise<T> {
  const engine = createEngine();
  try {
    return run(engine);
  } finally {
    nativeDispose(engine);
  }
}
