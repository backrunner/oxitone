import {
  ErrorCode,
  OxitoneError,
  vst3RecordingSelectionSchema,
  type Vst3InstanceTarget,
  type Vst3Recording,
} from "@oxitone/protocol";
import { RecordingTakeBuilder, type Vst3AutomationTake } from "../automation/recording-take.js";
import type { Session } from "./session.js";

export interface Vst3RecordingOptions {
  mode: "touch" | "write";
  parameterIds: readonly number[];
}
/** The same recorder can be owned by a Session or a revision-checked native Preview connection. */
export type Vst3RecordingHost = Pick<Session, "controlVst3Instance">;
/** Drains one bounded native recording. It never opens an audio device or mutates Project source. */
export class Vst3AutomationRecorder {
  private stopDeadline: number | undefined;
  private cancelled = false;
  private stopped = false;
  private fault: Error | undefined;
  private readonly completion: Promise<Vst3AutomationTake | undefined>;
  private constructor(
    private readonly session: Vst3RecordingHost,
    private readonly target: Vst3InstanceTarget,
    private readonly captureId: string,
    recording: Vst3Recording,
  ) {
    this.target = Object.freeze({ ...target });
    this.completion = this.collect(new RecordingTakeBuilder(this.target, captureId, recording));
    void this.completion.catch(() => {}); // Errors remain available through error and stop().
  }
  /** @internal Session owns the native instance capability. */
  static async start(
    session: Vst3RecordingHost,
    target: Vst3InstanceTarget,
    options: Vst3RecordingOptions,
  ): Promise<Vst3AutomationRecorder> {
    const selection = vst3RecordingSelectionSchema.parse({
      mode: options.mode,
      parameterIds: [...options.parameterIds].sort((a, b) => a - b),
    });
    const result = await session.controlVst3Instance(target, { kind: "startRecording", ...selection });
    const page = result.state.edits;
    if (!page?.recording) throw new OxitoneError(ErrorCode.PluginHostCrashed, "VST3 recording response missing");
    return new Vst3AutomationRecorder(session, target, page.captureId, page.recording);
  }
  get error(): Error | undefined {
    return this.fault;
  }
  get active(): boolean {
    return !this.stopped && !this.fault && !this.cancelled;
  }
  /** Stop while playback can still supply a boundary, then collect all remaining pages. */
  async stop(options: { timeoutMs?: number } = {}): Promise<Vst3AutomationTake> {
    const timeout = options.timeoutMs ?? 5000;
    if (!Number.isInteger(timeout) || timeout < 1 || timeout > 600000)
      throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Invalid recording stop timeout");
    this.stopDeadline ??= performance.now() + timeout;
    const result = await this.completion;
    if (this.cancelled || !result) throw new OxitoneError(ErrorCode.PluginTaskConflict, "VST3 recording was cancelled");
    return result;
  }
  async cancel(): Promise<void> {
    this.cancelled = true;
    await this.completion.catch(() => {});
  }
  private checkDeadline(): void {
    if (this.stopDeadline !== undefined && performance.now() >= this.stopDeadline)
      throw new OxitoneError(
        ErrorCode.PluginTaskConflict,
        "Recording stop requires a playing audio boundary before its deadline",
      );
  }
  private async collect(builder: RecordingTakeBuilder): Promise<Vst3AutomationTake | undefined> {
    let stopSent = false;
    try {
      while (!this.cancelled) {
        this.checkDeadline();
        const kind = this.stopDeadline !== undefined && !stopSent ? "stopEdits" : "readEdits";
        const response = await this.session.controlVst3Instance(
          this.target,
          {
            kind,
            captureId: this.captureId,
            fromSequence: builder.cursor,
          },
          {
            timeoutMs:
              this.stopDeadline === undefined ? 1000 : Math.max(1, Math.ceil(this.stopDeadline - performance.now())),
          },
        );
        if (this.cancelled) return undefined;
        this.checkDeadline();
        if (kind === "stopEdits") stopSent = true;
        if (!response.state.edits) throw new OxitoneError(ErrorCode.PluginHostCrashed, "VST3 recording page missing");
        const page = builder.accept(response.state.edits);
        if (page.status === "stopped" && builder.cursor === page.nextSequence) {
          this.stopped = true;
          return builder.finish();
        }
        if (builder.cursor === page.nextSequence) await new Promise((resolve) => setTimeout(resolve, 20));
      }
      return undefined;
    } catch (error) {
      this.fault = error instanceof Error ? error : new Error(String(error));
      throw this.fault;
    } finally {
      // A retired graph already owns helper teardown. Never redirect cleanup to the replacement.
      await this.session
        .controlVst3Instance(this.target, { kind: "discardEdits", captureId: this.captureId })
        .catch(() => {});
    }
  }
}
