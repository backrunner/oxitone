import {
  ErrorCode,
  OxitoneError,
  type EngineOptions,
  type MidiExportOptions,
  type MidiExportReport,
  type ProjectSnapshot,
  type RenderOptions,
  type RenderReport,
} from "@oxitone/protocol";
import {
  compile as nativeCompile,
  createEngine,
  dispose as nativeDispose,
  renderWav as nativeRenderWav,
  exportMidi as nativeExportMidi,
} from "oxitone";
import { Session, withTempEngine, type TransportPosition } from "./session.js";
import type { BarBeatPosition } from "./time-signature.js";

/** Native session lifecycle, separate from project authoring state. */
export abstract class ProjectPlayback {
  private activeSession?: Session;

  abstract snapshot(): ProjectSnapshot;
  abstract barBeatToBeats(position: BarBeatPosition): number;

  /**
   * Create a native engine and compile the current snapshot; the returned
   * session holds the `engineId` for transport, `setParameter`, offline
   * render and MIDI export. Replaces (and disposes) any previous session.
   */
  async compile(options?: EngineOptions): Promise<Session> {
    const engine = createEngine(options);
    try {
      nativeCompile(engine, this.snapshot());
    } catch (error) {
      nativeDispose(engine);
      throw error;
    }
    const previous = this.activeSession;
    this.activeSession = new Session(engine, () => this.snapshot(), (bar) =>
      this.barBeatToBeats({ bar }),
    );
    if (previous !== undefined) {
      await previous.dispose();
    }
    return this.activeSession;
  }

  /** The session from the last `compile()`/`play()`, if still active. */
  get session(): Session | undefined {
    return this.activeSession;
  }

  private requireSession(): Session {
    if (this.activeSession === undefined) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "no active session; call compile() or play() first",
      );
    }
    return this.activeSession;
  }

  /** One-shot offline WAV export on a temporary engine (04 §RenderOptions). */
  async renderWav(options: RenderOptions): Promise<RenderReport> {
    const snapshot = this.snapshot();
    return withTempEngine((engine) => nativeRenderWav(engine, snapshot, options));
  }

  /** One-shot SMF Type 1 export on a temporary engine. */
  async exportMidi(options: MidiExportOptions): Promise<MidiExportReport> {
    const snapshot = this.snapshot();
    return withTempEngine((engine) => nativeExportMidi(engine, snapshot, options));
  }

  /** Compile (if needed) and start playback; returns the active session. */
  async play(position?: TransportPosition): Promise<Session> {
    const session = this.activeSession ?? (await this.compile());
    await session.play(position);
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
