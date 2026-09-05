import type { TrackSpec } from "@oxitone/protocol";
import type { Channel } from "./channel.js";
import type { Pattern } from "./pattern.js";
import { PatternClipDraft, type PatternClip } from "./pattern-clip.js";
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

  /** IDs of the bound channels, in binding order. */
  get channelIds(): readonly string[] {
    return [...this.channelIdList];
  }

  /** Pattern clips placed on this track, in placement order. */
  get clips(): readonly PatternClip[] {
    return [...this.clipList];
  }

  /** Bind a channel to this track (idempotent; layering is allowed). */
  use(channel: Channel): this {
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
      sampleClipIds: [],
    };
    if (this.trackName !== undefined) {
      spec.name = this.trackName;
    }
    return spec;
  }
}
