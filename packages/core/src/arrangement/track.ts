import { ErrorCode, OxitoneError, trackSpecSchema, type TrackSpec } from "@oxitone/protocol";
import { parseAuthoring } from "../authoring-validation.js";
import type { Channel } from "../channels/channel.js";
import type { Pattern } from "../patterns/pattern.js";
import { PatternClipDraft, type PatternClip } from "./pattern-clip.js";
import { Sample, SampleClip, type SampleClipOptions } from "./sample.js";
import type { BarBeatPosition } from "../timing/time-signature.js";
import type { Project } from "../project/project.js";

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
  private muteValue = false;
  private soloValue = false;
  private midiChannelValue: number | undefined;
  private tempoValue: number | undefined;
  private restoredSpec?: TrackSpec;

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

  /** @internal Clips are attached in the saved membership order by the project restorer. */
  static fromSpec(project: Project, input: TrackSpec): Track {
    const spec = parseAuthoring(trackSpecSchema, input, "track");
    const track = new Track(project, spec.id, spec.name);
    track.channelIdList.push(...spec.channelIds);
    track.enabledValue = spec.enabled ?? true;
    track.muteValue = spec.mute ?? false;
    track.soloValue = spec.solo ?? false;
    track.tempoValue = spec.tempo;
    track.midiChannelValue = spec.midiChannel;
    track.restoredSpec = spec;
    return track;
  }

  get enabled(): boolean { return this.enabledValue; }
  /** Static local BPM (20..999); undefined follows the project clock. */
  get tempo(): number | undefined { return this.tempoValue; }
  set tempo(value: number | undefined) {
    this.project.assertMutable();
    if (value !== undefined && (!Number.isFinite(value) || value < 20 || value > 999)) {
      throw new OxitoneError(ErrorCode.TempoRange, "track tempo must be finite and in 20..999", {
        details: { path: "track.tempo" },
      });
    }
    this.tempoValue = value;
    if (this.restoredSpec !== undefined) this.restoredSpec.tempo = value;
    this.project.touch();
  }
  set enabled(value: boolean) {
    this.project.assertMutable();
    if (typeof value !== "boolean") {
      throw new OxitoneError(ErrorCode.InvalidProject, "track enabled must be a boolean", {
        details: { path: "track.enabled" },
      });
    }
    this.enabledValue = value;
    if (this.restoredSpec !== undefined) {
      if (value) delete this.restoredSpec.enabled; else this.restoredSpec.enabled = false;
    }
    this.project.touch();
  }

  get mute(): boolean { return this.muteValue; }
  set mute(value: boolean) {
    this.project.assertMutable();
    if (typeof value !== "boolean") throw new OxitoneError(ErrorCode.InvalidProject, "track mute must be a boolean");
    this.muteValue = value;
    if (this.restoredSpec !== undefined) {
      if (value) this.restoredSpec.mute = true; else delete this.restoredSpec.mute;
    }
    this.project.touch();
  }
  get solo(): boolean { return this.soloValue; }
  set solo(value: boolean) {
    this.project.assertMutable();
    if (typeof value !== "boolean") throw new OxitoneError(ErrorCode.InvalidProject, "track solo must be a boolean");
    this.soloValue = value;
    if (this.restoredSpec !== undefined) {
      if (value) this.restoredSpec.solo = true; else delete this.restoredSpec.solo;
    }
    this.project.touch();
  }

  /** Explicit MIDI channel (1..16); omitted means deterministic auto allocation. */
  get midiChannel(): number | undefined { return this.midiChannelValue; }
  set midiChannel(value: number | undefined) {
    this.project.assertMutable();
    if (value !== undefined && (!Number.isInteger(value) || value < 1 || value > 16)) {
      throw new OxitoneError(ErrorCode.InvalidProject, `midiChannel must be an integer in 1..16, got ${value}`, {
        details: { path: "track.midiChannel" },
      });
    }
    this.midiChannelValue = value;
    if (this.restoredSpec !== undefined) this.restoredSpec.midiChannel = value;
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
      this.project.assertMutable();
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
  /** @internal Playlist transactions preserve clip identity while moving membership. */
  detachClip(clip: PatternClip): void { const index = this.clipList.indexOf(clip); if (index >= 0) this.clipList.splice(index, 1); }
  /** @internal */
  detachSampleClip(clip: SampleClip): void { const index = this.sampleClipList.indexOf(clip); if (index >= 0) this.sampleClipList.splice(index, 1); }

  /** Wire form. */
  toSpec(): TrackSpec {
    const spec: TrackSpec = {
      ...this.restoredSpec,
      id: this.id,
      channelIds: [...this.channelIdList],
      patternClipIds: this.clipList.map((clip) => clip.id),
      sampleClipIds: this.sampleClipList.map((clip) => clip.id),
    };
    if (this.trackName !== undefined) {
      spec.name = this.trackName;
    }
    if (!this.enabledValue) spec.enabled = false;
    if (this.muteValue) spec.mute = true;
    if (this.soloValue) spec.solo = true;
    if (this.tempoValue !== undefined) spec.tempo = this.tempoValue;
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
