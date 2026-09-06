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
} from "@oxitone/protocol";
import {
  compile as nativeCompile,
  createEngine,
  dispose as nativeDispose,
  renderWav as nativeRenderWav,
  exportMidi as nativeExportMidi,
} from "oxitone";
import { Session, withTempEngine, type TransportPosition, type LoopRegion } from "./session.js";
import type { BarBeatPosition } from "./time-signature.js";

/** Native session lifecycle, separate from project authoring state. */
export type ProjectCompileOptions = EngineOptions & CompileOptions;

export abstract class ProjectPlayback {
  private activeSession?: Session;
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
    options = { assetBaseDir: this.assetBaseDir, ...options };
    const engine = createEngine(options);
    let snapshot: ProjectSnapshot;
    try {
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
    return withTempEngine((engine) => nativeRenderWav(engine, snapshot, { assetBaseDir: this.assetBaseDir, ...options }));
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
