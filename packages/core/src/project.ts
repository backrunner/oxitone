import {
  ErrorCode,
  ID_PREFIXES,
  OxitoneError,
  type EntityId,
  type ProjectSnapshot,
  type TimeSignatureSegment,
} from "@oxitone/protocol";
import { Channel, type ChannelOptions } from "./channel.js";
import { Sample, type SampleOptions } from "./sample.js";
import { IdGenerator } from "./ids.js";
import { MixerChannel, type MixerChannelOptions } from "./mixer-channel.js";
import {
  AutomationLane,
  type AutomationLaneOptions,
  type AutomationLaneTarget,
} from "./automation/lane.js";
import { AutomationSource } from "./automation/source.js";
import type { Pattern } from "./pattern.js";
import { PatternClip } from "./pattern-clip.js";
import { ProjectPlayback } from "./project-playback.js";
import { snapshotProject } from "./project-snapshot.js";
import { Track } from "./track.js";
import { TempoMap, type TempoCurve, type TempoSegmentInput } from "./tempo-map.js";
import { TimeSignatureMap, type BarBeatPosition } from "./time-signature.js";

/** Options for {@link Project}. */
export interface ProjectOptions {
  id?: EntityId;
  name?: string;
  sampleRate?: number;
  blockSize?: number;
  seed?: number;
}

/** A named beat position on the project timeline. */
export interface Marker {
  id: EntityId;
  name: string;
  startBeat: number;
}

const DEFAULT_SAMPLE_RATE = 48_000;
const DEFAULT_BLOCK_SIZE = 128;

function checkPositiveInteger(value: number, path: string): void {
  if (!Number.isInteger(value) || value < 1) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `${path} must be an integer >= 1, got ${value}`,
      { details: { path } },
    );
  }
}

/**
 * Authoring root: owns the tempo map, time-signature map, markers, tracks,
 * channels, mixer buses and Master. Every mutation bumps
 * `revision`; `snapshot()` emits a protocol-validated immutable snapshot.
 */
export class Project extends ProjectPlayback {
  readonly id: EntityId;
  readonly sampleRate: number;
  readonly blockSize: number;
  readonly seed: number;
  readonly master: MixerChannel;
  private readonly projectName?: string;
  private readonly ids: IdGenerator;
  private readonly tempos = new TempoMap();
  private readonly signatures = new TimeSignatureMap();
  private readonly masterId: EntityId;
  private readonly markerList: Marker[] = [];
  private readonly trackList: Track[] = [];
  private readonly channelList: Channel[] = [];
  private readonly mixerChannelList: MixerChannel[] = [];
  private readonly sampleList: Sample[] = [];
  private readonly automationLaneList: AutomationLane[] = [];
  private readonly patternsById = new Map<string, Pattern>();
  private readonly entityIds = new Set<string>();
  private revisionCounter = 0;

  constructor(options: ProjectOptions = {}) {
    super();
    this.seed = options.seed ?? 0;
    if (!Number.isInteger(this.seed) || this.seed < 0) {
      throw new OxitoneError(ErrorCode.InvalidProject, `seed must be an integer >= 0, got ${this.seed}`, {
        details: { path: "seed" },
      });
    }
    this.sampleRate = options.sampleRate ?? DEFAULT_SAMPLE_RATE;
    this.blockSize = options.blockSize ?? DEFAULT_BLOCK_SIZE;
    checkPositiveInteger(this.sampleRate, "sampleRate");
    checkPositiveInteger(this.blockSize, "blockSize");
    this.ids = new IdGenerator(this.seed);
    this.id = options.id ?? this.ids.next("prj_");
    this.masterId = "mix_master";
    this.entityIds.add(this.id).add(this.masterId);
    this.master = new MixerChannel(this, this.masterId, { name: "Master" });
    this.mixerChannelList.push(this.master);
    if (options.name !== undefined) {
      this.projectName = options.name;
    }
  }

  get name(): string | undefined {
    return this.projectName;
  }

  /** Monotonically increasing mutation counter. */
  get revision(): number {
    return this.revisionCounter;
  }

  /** ID of the undeletable Master mixer channel. */
  get masterMixerChannelId(): EntityId {
    return this.masterId;
  }

  /** Master followed by user buses in creation order. */
  get mixerChannels(): readonly MixerChannel[] { return [...this.mixerChannelList]; }

  addMixerChannel(options: MixerChannelOptions = {}): MixerChannel {
    const bus = new MixerChannel(this, this.ids.next(ID_PREFIXES.mixerChannel), options);
    this.mixerChannelList.push(bus);
    this.entityIds.add(bus.id);
    this.touch();
    return bus;
  }

  /** @internal Allocate and register a project entity ID for clip builders. */
  nextEntityId(prefix: string): EntityId { return this.claimId(prefix); }

  /** Register an immutable sample asset reference; decoding occurs in Rust prepare. */
  addSample(options: SampleOptions): Sample {
    const sample = new Sample(this.claimId(ID_PREFIXES.sample), options);
    this.sampleList.push(sample);
    this.touch();
    return sample;
  }

  get samples(): readonly Sample[] { return [...this.sampleList]; }
  get sampleClips() { return this.trackList.flatMap((track) => track.sampleClips); }

  /** @internal Resolve routing IDs against buses owned by this project. */
  requireMixerChannel(id: EntityId): MixerChannel {
    const bus = this.mixerChannelList.find((entry) => entry.id === id);
    if (bus === undefined) {
      throw new OxitoneError(ErrorCode.InvalidProject, `unknown mixerChannelId: ${id}`, {
        details: { path: "channel.mixerChannelId" },
      });
    }
    return bus;
  }

  /** @internal Bump the revision after any authoring mutation. */
  touch(): void {
    this.revisionCounter += 1;
  }

  /** Replace the tempo map with a static tempo (`step` curve by default). */
  setTempo(bpm: number, curve?: TempoCurve): this {
    this.tempos.set(bpm, curve);
    this.touch();
    return this;
  }

  /** Append a tempo segment; start beats must strictly increase. */
  addTempoSegment(segment: TempoSegmentInput): this {
    this.tempos.add(segment);
    this.touch();
    return this;
  }

  /** Current tempo segments (defensive copy). */
  get tempoMap(): TempoSegmentInput[] {
    return this.tempos.list();
  }

  /** Replace the time-signature map with a single signature from bar 1. */
  setTimeSignature(numerator: number, denominator: number): this {
    this.signatures.set(numerator, denominator);
    this.touch();
    return this;
  }

  /** Append a time-signature change at a bar boundary. */
  addTimeSignature(segment: TimeSignatureSegment): this {
    this.signatures.add(segment);
    this.touch();
    return this;
  }

  /** Current time-signature segments (defensive copy). */
  get timeSignatureMap(): TimeSignatureSegment[] {
    return this.signatures.list();
  }

  /** Convert a bar/beat position to absolute project beats. */
  barBeatToBeats(position: BarBeatPosition): number {
    return this.signatures.toBeats(position);
  }

  /** Beats contained in one bar at a given bar. */
  beatsPerBarAt(bar: number): number { return this.signatures.beatsPerBarAt(bar); }

  /** Static tempo at a beat, used only for authoring fit helpers. */
  tempoAt(beat: number): number {
    const segments = this.tempos.list();
    let bpm = segments[0]?.bpm ?? 120;
    for (const segment of segments) {
      if (segment.startBeat > beat) break;
      bpm = segment.bpm;
    }
    return bpm;
  }

  /** Add a named marker at a beat position; returns the marker. */
  addMarker(name: string, beat: number): Marker {
    if (!Number.isFinite(beat) || beat < 0) {
      throw new OxitoneError(ErrorCode.InvalidProject, `marker beat must be >= 0, got ${beat}`, {
        details: { path: "markers.startBeat" },
      });
    }
    const marker: Marker = { id: this.claimId("mrk_"), name, startBeat: beat };
    this.markerList.push(marker);
    this.touch();
    return { ...marker };
  }

  /** Markers in insertion order (defensive copy). */
  get markers(): readonly Marker[] {
    return this.markerList.map((marker) => ({ ...marker }));
  }

  /** Create a track. */
  addTrack(name?: string): Track {
    const track = new Track(this, this.claimId(ID_PREFIXES.track), name);
    this.trackList.push(track);
    this.touch();
    return track;
  }

  /** Tracks in creation order. */
  get tracks(): readonly Track[] {
    return [...this.trackList];
  }

  /**
   * Create a channel. Without `mixerChannelId` it routes to Master; without
   * `instrument` it uses the built-in wavetable instrument.
   */
  addChannel(options: ChannelOptions = {}): Channel {
    const channel = new Channel(this.ids.next(ID_PREFIXES.channel), options, this.masterId, this);
    this.channelList.push(channel);
    this.entityIds.add(channel.id);
    this.touch();
    return channel;
  }

  /** Channels in creation order. */
  get channels(): readonly Channel[] {
    return [...this.channelList];
  }

  /**
   * Bind an automation source to a target parameter (04-api-contracts.md
   * `AutomationLaneSpec`). The target entity must exist in this project; the
   * project entity itself only exposes `tempo`, and at most one tempo lane is
   * allowed (`TempoAutomationConflict`).
   */
  addAutomationLane(
    target: AutomationLaneTarget,
    source: AutomationSource,
    options: AutomationLaneOptions = {},
  ): AutomationLane {
    if (!(source instanceof AutomationSource)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "lane source must be an AutomationSource", {
        details: { path: "automation.source" },
      });
    }
    if (!this.entityIds.has(target.entityId)) {
      throw new OxitoneError(
        ErrorCode.AutomationTargetInvalid,
        `unknown automation target entity: ${target.entityId}`,
        { details: { path: "automation.target.entityId" } },
      );
    }
    if (target.entityId === this.id) {
      if (target.parameterId !== "tempo") {
        throw new OxitoneError(
          ErrorCode.AutomationTargetInvalid,
          `project exposes only the 'tempo' parameter, got '${target.parameterId}'`,
          { details: { path: "automation.target.parameterId" } },
        );
      }
      if (this.automationLaneList.some((lane) => lane.target.entityId === this.id)) {
        throw new OxitoneError(
          ErrorCode.TempoAutomationConflict,
          "a tempo automation lane already exists for this project",
          { details: { path: "automation.target" } },
        );
      }
    }
    const lane = new AutomationLane(this.claimId(ID_PREFIXES.automation), target, source, options);
    this.automationLaneList.push(lane);
    this.touch();
    return lane;
  }

  /** Automation lanes in creation order. */
  get automationLanes(): readonly AutomationLane[] {
    return [...this.automationLaneList];
  }

  /** @internal Create and attach a pattern clip; called by the draft API. */
  createPatternClip(track: Track, pattern: Pattern, startBeat: number): PatternClip {
    this.registerPattern(pattern);
    const clip = new PatternClip(this, track, pattern, this.claimId("pcl_"), startBeat);
    track.attachClip(clip);
    this.touch();
    return clip;
  }

  private registerPattern(pattern: Pattern): void {
    const existing = this.patternsById.get(pattern.id);
    if (existing !== undefined && existing !== pattern) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `duplicate pattern id: ${pattern.id}`,
        { details: { path: "patterns.id" } },
      );
    }
    this.patternsById.set(pattern.id, pattern);
  }

  private claimId(prefix: string): EntityId {
    const id = this.ids.next(prefix);
    this.entityIds.add(id);
    return id;
  }

  /**
   * Immutable project state for the engine, validated against the protocol
   * `projectSnapshotSchema` before returning. ID-carrying collections are
   * emitted in ascending ID order, matching the canonical encoding rules.
   */
  snapshot(): ProjectSnapshot {
    return snapshotProject(this, [...this.patternsById.values()], this.tempos.toWire());
  }
}
