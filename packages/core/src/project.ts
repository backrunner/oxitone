import {
  beatToWire,
  ErrorCode,
  ID_PREFIXES,
  OxitoneError,
  PROTOCOL_VERSION,
  projectSnapshotSchema,
  type EngineOptions,
  type EntityId,
  type MarkerSpec,
  type MidiExportOptions,
  type MidiExportReport,
  type ProjectSnapshot,
  type RenderOptions,
  type RenderReport,
  type TempoSegment,
  type TimeSignatureSegment,
} from "@oxitone/protocol";
import {
  compile as nativeCompile,
  createEngine,
  dispose as nativeDispose,
  renderWav as nativeRenderWav,
  exportMidi as nativeExportMidi,
} from "oxitone";
import { Channel, type ChannelOptions } from "./channel.js";
import { IdGenerator } from "./ids.js";
import {
  AutomationLane,
  type AutomationLaneOptions,
  type AutomationLaneTarget,
} from "./automation/lane.js";
import { AutomationSource } from "./automation/source.js";
import type { Pattern } from "./pattern.js";
import { PatternClip } from "./pattern-clip.js";
import { Session, withTempEngine, type TransportPosition } from "./session.js";
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
 * channels, and the implicit Master mixer channel. Every mutation bumps
 * `revision`; `snapshot()` emits a protocol-validated immutable snapshot.
 */
export class Project {
  readonly id: EntityId;
  readonly sampleRate: number;
  readonly blockSize: number;
  readonly seed: number;
  private readonly projectName?: string;
  private readonly ids: IdGenerator;
  private readonly tempos = new TempoMap();
  private readonly signatures = new TimeSignatureMap();
  private readonly masterId: EntityId;
  private readonly markerList: Marker[] = [];
  private readonly trackList: Track[] = [];
  private readonly channelList: Channel[] = [];
  private readonly automationLaneList: AutomationLane[] = [];
  private readonly patternsById = new Map<string, Pattern>();
  private readonly entityIds = new Set<string>();
  private revisionCounter = 0;
  private activeSession?: Session;

  constructor(options: ProjectOptions = {}) {
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
   * `instrument` it uses the M1 placeholder ref (see `DEFAULT_INSTRUMENT`).
   */
  addChannel(options: ChannelOptions = {}): Channel {
    const channel = new Channel(this.claimId(ID_PREFIXES.channel), options, this.masterId, this);
    if (options.mixerChannelId !== undefined && options.mixerChannelId !== this.masterId) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        `unknown mixerChannelId: ${options.mixerChannelId} (only Master exists in M1)`,
        { details: { path: "channel.mixerChannelId" } },
      );
    }
    this.channelList.push(channel);
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
    const byId = <T extends { id: string }>(items: T[]): T[] =>
      [...items].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    const clips = byId(this.trackList.flatMap((track) => track.clips));
    const markers: MarkerSpec[] = byId(this.markerList).map((marker) => ({
      id: marker.id,
      name: marker.name,
      startBeat: beatToWire(marker.startBeat),
    }));
    const tempoMap: TempoSegment[] = this.tempos.toWire();
    const snapshot = {
      protocolVersion: PROTOCOL_VERSION,
      revision: String(this.revisionCounter),
      id: this.id,
      ...(this.projectName !== undefined ? { name: this.projectName } : {}),
      sampleRate: this.sampleRate,
      blockSize: this.blockSize,
      seed: this.seed,
      tempoMap,
      timeSignatureMap: this.signatures.list(),
      markers,
      tracks: byId(this.trackList).map((track) => track.toSpec()),
      patterns: byId([...this.patternsById.values()]).map((pattern) => pattern.toSpec()),
      patternClips: clips.map((clip) => clip.toSpec()),
      sampleClips: [],
      samples: [],
      channels: byId(this.channelList).map((channel) => channel.toSpec()),
      automation: byId(this.automationLaneList).map((lane) => lane.toSpec()),
      mixerChannels: [
        {
          id: this.masterId,
          name: "Master",
          level: 1,
          balance: 0,
          inserts: [],
          sends: [],
        },
      ],
    };
    return projectSnapshotSchema.parse(snapshot);
  }

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
