import {
  automationClipSpecSchema,
  beatFromWire,
  beatToWire,
  ErrorCode,
  OxitoneError,
  type AutomationClipSpec,
  type EntityId,
} from "@oxitone/protocol";
import type { Project } from "../project/project.js";
import type { Track } from "../arrangement/track.js";

/** Immutable wire identity with validated, revisioned placement changes. */
export class AutomationClip {
  private spec: AutomationClipSpec;
  constructor(
    private readonly project: Project,
    input: AutomationClipSpec,
  ) {
    const spec = automationClipSpecSchema.parse(input);
    if (beatFromWire(spec.startBeat) < 0 || spec.durationBeats === undefined || beatFromWire(spec.durationBeats) <= 0) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "Automation clips require a nonnegative start and positive duration",
      );
    }
    this.spec = structuredClone(spec);
  }
  get id(): EntityId {
    return this.spec.id;
  }
  get laneId(): EntityId {
    return this.spec.laneId;
  }
  get trackId(): EntityId {
    return this.spec.trackId;
  }
  get startBeat(): number {
    return beatFromWire(this.spec.startBeat);
  }
  get durationBeats(): number {
    return beatFromWire(this.spec.durationBeats!);
  }
  set durationBeats(value: number) {
    this.project.assertMutable();
    if (!Number.isFinite(value) || value <= 0)
      throw new OxitoneError(ErrorCode.InvalidProject, "Automation clip duration must be positive");
    this.spec.durationBeats = beatToWire(value);
    this.project.touch();
  }
  get enabled(): boolean {
    return this.spec.enabled !== false;
  }
  set enabled(value: boolean) {
    this.project.assertMutable();
    this.spec.enabled = value;
    this.project.touch();
  }
  relocate(track: Track, startBeat: number): void {
    this.project.assertMutable();
    if (
      !this.project.tracks.includes(track) ||
      track.tempo !== undefined ||
      !Number.isFinite(startBeat) ||
      startBeat < 0
    ) {
      throw new OxitoneError(ErrorCode.InvalidProject, "invalid automation placement");
    }
    this.spec.trackId = track.id;
    this.spec.startBeat = beatToWire(startBeat);
    this.project.touch();
  }
  toSpec(): AutomationClipSpec {
    return structuredClone(this.spec);
  }
}
