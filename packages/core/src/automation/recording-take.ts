import {
  ErrorCode,
  OxitoneError,
  canonicalEncode,
  vst3EditPageSchema,
  type Vst3EditPage,
  type Vst3Recording,
  type Vst3InstanceTarget,
} from "@oxitone/protocol";
import { AutomationSource } from "./source.js";
import { recordedSource, type Vst3RecordedSpan } from "./recording-source.js";

/** A complete recording; applying its source is an explicit authoring operation. */
export interface Vst3AutomationTake {
  readonly target: Readonly<Vst3InstanceTarget>;
  readonly recording: Readonly<{ mode: Vst3Recording["mode"]; sampleRate: number; parameterIds: readonly number[] }>;
  readonly spans: readonly Vst3RecordedSpan[];
  source(parameterId: number, base: AutomationSource): AutomationSource;
}
class CompletedTake implements Vst3AutomationTake {
  readonly recording: Vst3AutomationTake["recording"];
  readonly spans: readonly Vst3RecordedSpan[];
  /** @internal Returned only after a successful native stop boundary. */
  constructor(
    readonly target: Readonly<Vst3InstanceTarget>,
    recording: Vst3Recording,
    spans: readonly Vst3RecordedSpan[],
  ) {
    this.target = Object.freeze({ ...target });
    this.recording = Object.freeze({ ...recording, parameterIds: Object.freeze([...recording.parameterIds]) });
    this.spans = Object.freeze(spans.map((span) => Object.freeze({ ...span })));
    Object.freeze(this);
  }
  /** Project-beat overlay. Map lane/clip-local clocks explicitly before binding this source. */
  source(parameterId: number, base: AutomationSource): AutomationSource {
    if (!this.recording.parameterIds.includes(parameterId))
      throw new OxitoneError(ErrorCode.AutomationTargetInvalid, "Parameter is not part of this recording");
    return recordedSource(this.spans, parameterId, base);
  }
}

/** @internal Cursor and budget checks are shared by the asynchronous recorder and focused tests. */
export class RecordingTakeBuilder {
  cursor = 0;
  private audioSequence = 0;
  private highWater = 0;
  private readonly spans: Vst3RecordedSpan[] = [];
  private readonly last = new Map<number, { index: number; endFrame: number; continuousEnd: number }>();
  private completed = false;
  constructor(
    readonly target: Vst3InstanceTarget,
    readonly captureId: string,
    readonly recording: Vst3Recording,
  ) {}
  accept(input: Vst3EditPage): Vst3EditPage {
    const page = vst3EditPageSchema.parse(input);
    if (
      page.captureId !== this.captureId ||
      page.firstSequence !== this.cursor ||
      page.nextSequence < this.highWater ||
      canonicalEncode(page.recording) !== canonicalEncode(this.recording)
    )
      throw new OxitoneError(ErrorCode.PluginTaskConflict, "VST3 recording page identity or cursor changed");
    if (page.error) throw new OxitoneError(ErrorCode[page.error.code], page.error.message);
    for (const event of page.events) {
      if (event.position.audioSequence < this.audioSequence)
        throw new OxitoneError(ErrorCode.PluginTaskConflict, "VST3 recording audio clock moved backwards");
      this.audioSequence = event.position.audioSequence;
      if (event.kind !== "sample") continue;
      const position = event.position.transport;
      const previous = this.last.get(event.parameterId);
      let start = position.projectBeat;
      const end = start + (event.frames * position.tempo) / (60 * this.recording.sampleRate);
      const old = previous ? this.spans[previous.index] : undefined;
      const adjacent =
        old &&
        previous?.endFrame === position.projectFrame &&
        previous.continuousEnd === position.continuousFrame &&
        !event.position.reset &&
        Math.abs(old.end - start) < 1e-9;
      if (adjacent) start = old.end;
      if (!Number.isFinite(end) || end <= start)
        throw new OxitoneError(ErrorCode.AutomationRange, "VST3 recording interval cannot be represented in beats");
      let index = this.spans.length;
      if (adjacent && old.value === event.value) {
        index = previous!.index;
        this.spans[index] = { ...old, end };
      } else {
        if (index >= 32768)
          throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 recording exceeds 32768 value intervals");
        this.spans.push({ parameterId: event.parameterId, start, end, value: event.value });
      }
      this.last.set(event.parameterId, {
        index,
        endFrame: position.projectFrame + event.frames,
        continuousEnd: position.continuousFrame + event.frames,
      });
    }
    this.cursor = page.firstSequence + page.events.length;
    this.highWater = page.nextSequence;
    this.completed = page.status === "stopped" && this.cursor === page.nextSequence;
    return page;
  }
  finish(): Vst3AutomationTake {
    if (!this.completed)
      throw new OxitoneError(ErrorCode.PluginTaskConflict, "VST3 recording is not fully stopped and collected");
    return new CompletedTake(this.target, this.recording, this.spans);
  }
}
