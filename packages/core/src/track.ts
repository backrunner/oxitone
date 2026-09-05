import { ErrorCode, OxitoneError, type TrackSpec } from "@oxitone/protocol";
import type { Channel } from "./channel.js";
import type { Pattern } from "./pattern.js";
import { PatternClipDraft, type PatternClip } from "./pattern-clip.js";
import { Sample, SampleClip, type SampleClipOptions } from "./sample.js";
import type { BarBeatPosition } from "./time-signature.js";
import type { Project } from "./project.js";

/**
 * Arrangement container. A track produces no sound itself; it binds one or
 * more channels and owns pattern clips.
 */
export class Track {
  readonly id: string;
  private readonly project: Project;
  private readonly trackName?: string;
  private readonly channelIdList: string[] = [];
  private readonly clipList: PatternClip[] = [];
  private readonly sampleClipList: SampleClip[] = [];
  private enabledValue = true;
  private midiChannelValue: number | undefined;

  /** @internal Use `project.addTrack(...)` instead. */
  constructor(project: Project, id: string, name?: string) {
    this.project = project;
    this.id = id;
    if (name !== undefined) {
      this.trackName = name;
    }
  }

  get name(): string | undefined {
    return this.trackName;
  }

  get enabled(): boolean { return this.enabledValue; }
  set enabled(value: boolean) {
    if (typeof value !== "boolean") {
      throw new OxitoneError(ErrorCode.InvalidProject, "track enabled must be a boolean", {
        details: { path: "track.enabled" },
      });
    }
    this.enabledValue = value;
    this.project.touch();
  }

  /** Explicit MIDI channel (1..16); omitted means deterministic auto allocation. */
  get midiChannel(): number | undefined { return this.midiChannelValue; }
  set midiChannel(value: number | undefined) {
    if (value !== undefined && (!Number.isInteger(value) || value < 1 || value > 16)) {
      throw new OxitoneError(ErrorCode.InvalidProject, `midiChannel must be an integer in 1..16, got ${value}`, {
        details: { path: "track.midiChannel" },
      });
    }
    this.midiChannelValue = value;
    this.project.touch();
  }

  /** IDs of the bound channels, in binding order. */
  get channelIds(): readonly string[] {
    return [...this.channelIdList];
  }

  /** Pattern clips placed on this track, in placement order. */
  get clips(): readonly PatternClip[] {
    return [...this.clipList];
  }

  get sampleClips(): readonly SampleClip[] { return [...this.sampleClipList]; }

  /** Start placing a Sample; finish with `.at({ bar, beat })`. */
  sample(sample: Sample): SampleClipDraft {
    if (!this.project.samples.includes(sample)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "sample must belong to this project", {
        details: { path: "sampleClip.sampleId" },
      });
    }
    return new SampleClipDraft(this.project, this, sample);
  }

  /** @internal */
  attachSampleClip(clip: SampleClip): void { this.sampleClipList.push(clip); }

  /** Bind a channel to this track (idempotent; layering is allowed). */
  use(channel: Channel): this {
    if (!this.project.channels.includes(channel)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "channel must belong to this project", {
        details: { path: "track.channelIds" },
      });
    }
    if (!this.channelIdList.includes(channel.id)) {
      this.channelIdList.push(channel.id);
      this.project.touch();
    }
    return this;
  }

  /** Start placing `pattern` on this track; finish with `.at({ bar, beat })`. */
  pattern(pattern: Pattern): PatternClipDraft {
    return new PatternClipDraft(this.project, this, pattern);
  }

  /** Alias for {@link pattern}. */
  add(pattern: Pattern): PatternClipDraft {
    return this.pattern(pattern);
  }

  /** @internal Attach a placed clip; called by `PatternClipDraft.at()`. */
  attachClip(clip: PatternClip): void {
    this.clipList.push(clip);
  }

  /** Wire form. */
  toSpec(): TrackSpec {
    const spec: TrackSpec = {
      id: this.id,
      channelIds: [...this.channelIdList],
      patternClipIds: this.clipList.map((clip) => clip.id),
      sampleClipIds: this.sampleClipList.map((clip) => clip.id),
    };
    if (this.trackName !== undefined) {
      spec.name = this.trackName;
    }
    if (!this.enabledValue) spec.enabled = false;
    if (this.midiChannelValue !== undefined) spec.midiChannel = this.midiChannelValue;
    return spec;
  }
}

/** Intermediate builder returned by `track.sample(sample)` before placement. */
export class SampleClipDraft {
  private placed = false;
  constructor(private readonly project: Project, private readonly track: Track, private readonly sampleValue: Sample) {}
  at(position: BarBeatPosition, options: SampleClipOptions = {}): SampleClip {
    if (this.placed) throw new OxitoneError(ErrorCode.InvalidProject, "sample clip draft is already placed");
    const clip = this.project.createSampleClip(this.track, this.sampleValue, position, options);
    this.placed = true;
    return clip;
  }
}
