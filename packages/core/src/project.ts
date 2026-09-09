import { resolve } from "#platform-path";
import {
  ErrorCode,
  ID_PREFIXES,
  OxitoneError,
  type EntityId,
  type ProjectSnapshot,
  type CompileOptions,
} from "@oxitone/protocol";
import { Channel, type ChannelOptions } from "./channel.js";
import { Sample, SampleClip, type SampleClipOptions, type SampleOptions } from "./sample.js";
import { IdGenerator } from "./ids.js";
import { MixerChannel, type MixerChannelOptions } from "./mixer-channel.js";
import {
  AutomationLane,
  type AutomationLaneOptions,
  type AutomationLaneTarget,
} from "./automation/lane.js";
import { AutomationClip } from "./automation/clip.js";
import type { AutomationSource } from "./automation/source.js";
import { ProjectAutomation } from "./project-automation.js";
import { registerPattern } from "./pattern-registration.js";
import type { Pattern } from "./pattern.js";
import { PatternClip } from "./pattern-clip.js";
import { ProjectTimeline } from "./project-timeline.js";
import { snapshotProject } from "./project-snapshot.js";
import { Track } from "./track.js";
import type { BarBeatPosition } from "./time-signature.js";
import { saveProject, loadProject, type SaveProjectOptions } from "#project-files";
import { parseRestorableSnapshot } from "./project-restore.js";
import { restoreEntities } from "./project-hydrate.js";
import { arrange } from "./project-arrangement.js";
import { configure } from "./project-edit.js";

export type { Marker } from "./project-timeline.js";

/** Options for {@link Project}. */
export interface ProjectOptions {
  id?: EntityId;
  name?: string;
  sampleRate?: number;
  blockSize?: number;
  seed?: number;
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
export class Project extends ProjectTimeline {
  readonly id: EntityId;
  readonly sampleRate: number;
  readonly blockSize: number;
  readonly seed: number;
  readonly master: MixerChannel;
  private readonly projectName?: string;
  private readonly ids: IdGenerator;
  private readonly masterId: EntityId;
  private readonly trackList: Track[] = [];
  private readonly channelList: Channel[] = [];
  private readonly mixerChannelList: MixerChannel[] = [];
  private readonly sampleList: Sample[] = [];
  private readonly automationStore = new ProjectAutomation(this, prefix => this.claimId(prefix), () => this.entityIds);
  private readonly patternsById = new Map<string, Pattern>();
  private readonly entityIds = new Set<string>();
  private revisionCounter = 0n;
  private implicitMaster = false;

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
  /** Apply a Playlist placement, move or removal. Indices follow builder creation order. */
  arrange(edit: import("@oxitone/protocol").ArrangementEdit): this { arrange(this, edit); return this; }
  /** Edit mixer, Track, tempo or plugin configuration using current builder order. */
  configure(edit: import("@oxitone/protocol").ProjectEdit): this { configure(this, edit); return this; }

  /** Monotonically increasing mutation counter. */
  get revision(): number {
    if (this.revisionCounter > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new OxitoneError(ErrorCode.InvalidProject, "revision exceeds safe integer range; use revisionBigInt");
    }
    return Number(this.revisionCounter);
  }

  get revisionBigInt(): bigint { return this.revisionCounter; }

  /** @internal Reject exhausted revisions before mutating any builder. */
  assertMutable(): void {
    if (this.revisionCounter === 0xffff_ffff_ffff_ffffn) {
      throw new OxitoneError(ErrorCode.InvalidProject, "project revision exhausted");
    }
  }

  /** ID of the undeletable Master mixer channel. */
  get masterMixerChannelId(): EntityId {
    return this.masterId;
  }

  /** Master followed by user buses in creation order. */
  get mixerChannels(): readonly MixerChannel[] { return [...this.mixerChannelList]; }

  addMixerChannel(options: MixerChannelOptions = {}): MixerChannel {
    this.assertMutable();
    const bus = new MixerChannel(this, this.ids.nextUnused(ID_PREFIXES.mixerChannel, this.entityIds), options);
    this.mixerChannelList.push(bus);
    this.entityIds.add(bus.id);
    this.touch();
    return bus;
  }

  /** @internal Validate before registering or attaching a sample clip. */
  createSampleClip(track: Track, sample: Sample, position: BarBeatPosition, options: SampleClipOptions): SampleClip {
    this.assertMutable();
    const clip = new SampleClip(this, track, sample, this.ids.nextUnused("scl_", this.entityIds), position, options);
    this.entityIds.add(clip.id);
    track.attachSampleClip(clip);
    this.touch();
    return clip;
  }

  /** Register an immutable sample asset reference; decoding occurs in Rust prepare. */
  addSample(options: SampleOptions): Sample {
    this.assertMutable();
    const id = options.id ?? this.ids.nextUnused(ID_PREFIXES.sample, this.entityIds);
    if (this.entityIds.has(id) || this.patternsById.has(id)) {
      throw new OxitoneError(ErrorCode.InvalidProject, `duplicate sample id: ${id}`, {
        details: { path: "sample.id" },
      });
    }
    const sample = new Sample(id, options);
    this.sampleList.push(sample);
    this.entityIds.add(sample.id);
    this.touch();
    return sample;
  }

  get samples(): readonly Sample[] { return [...this.sampleList]; }

  /** Import a detached resource descriptor, preserving exact rational musical length. */
  importSampleRef(ref: import("@oxitone/protocol").SampleRef, id?: string): Sample {
    this.assertMutable();
    const next = id ?? this.ids.nextUnused(ID_PREFIXES.sample, this.entityIds);
    if (this.entityIds.has(next)) throw new OxitoneError(ErrorCode.InvalidProject, `duplicate sample ID: ${next}`);
    const sample = Sample.fromSpec({ ...ref, id: next });
    this.sampleList.push(sample);
    this.entityIds.add(next);
    this.touch();
    return sample;
  }
  get patterns(): readonly Pattern[] { return [...this.patternsById.values()]; }
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
    this.assertMutable();
    this.revisionCounter += 1n;
  }

  /** Create a track. */
  addTrack(name?: string): Track {
    this.assertMutable();
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
    this.assertMutable();
    const channel = new Channel(this.ids.nextUnused(ID_PREFIXES.channel, this.entityIds), options, this.masterId, this);
    this.channelList.push(channel);
    this.entityIds.add(channel.id);
    this.touch();
    return channel;
  }

  /** Channels in creation order. */
  get channels(): readonly Channel[] {
    return [...this.channelList];
  }

  addAutomationLane(target: AutomationLaneTarget, source: AutomationSource, options: AutomationLaneOptions = {}): AutomationLane {
    return this.automationStore.addAutomationLane(target, source, options);
  }
  get automationLanes(): readonly AutomationLane[] { return this.automationStore.automationLanes; }
  get automationClips(): readonly AutomationClip[] { return this.automationStore.automationClips; }
  createAutomationClip(lane: AutomationLane, track: Track, startBeat: number, durationBeats?: number): AutomationClip {
    return this.automationStore.createAutomationClip(lane, track, startBeat, durationBeats);
  }
  removeAutomationClip(clip: AutomationClip): void { this.automationStore.removeAutomationClip(clip); }
  removeAutomationLane(lane: AutomationLane): void { this.automationStore.removeAutomationLane(lane); }

  /** @internal Create and attach a pattern clip; called by the draft API. */
  createPatternClip(track: Track, pattern: Pattern, startBeat: number): PatternClip {
    this.assertMutable();
    registerPattern(this, pattern, this.patternsById, this.entityIds);
    const clip = new PatternClip(this, track, pattern, this.claimId("pcl_"), startBeat);
    track.attachClip(clip);
    this.touch();
    return clip;
  }

  protected claimId(prefix: string): EntityId {
    const id = this.ids.nextUnused(prefix, this.entityIds);
    this.entityIds.add(id);
    return id;
  }

  /**
   * Immutable project state for the engine, validated against the protocol
   * `projectSnapshotSchema` before returning. ID-carrying collections are
   * emitted in ascending ID order, matching the canonical encoding rules.
   */
  snapshot(): ProjectSnapshot {
    return snapshotProject(this, [...this.patternsById.values()], this.tempoSegments());
  }

  /** Restore editable builders without opening resources or instantiating plugins. */
  static fromSnapshot(input: ProjectSnapshot, options: CompileOptions = {}): Project {
    const { snapshot, ids } = parseRestorableSnapshot(input);
    const project = new Project({ id: snapshot.id, ...(snapshot.name === undefined ? {} : { name: snapshot.name }),
      sampleRate: snapshot.sampleRate, blockSize: snapshot.blockSize, seed: snapshot.seed });
    if (options.assetBaseDir !== undefined) {
      if (!options.assetBaseDir || options.assetBaseDir.includes("\0")) {
        throw new OxitoneError(ErrorCode.InvalidProject, "assetBaseDir must be a nonempty local path");
      }
      project.projectAssetBaseDir = resolve(options.assetBaseDir);
    }
    project.restoreTimeline(snapshot);
    const master = snapshot.mixerChannels.find((bus) => bus.id === "mix_master");
    project.implicitMaster = master === undefined;
    if (master !== undefined) project.master.restoreSpec(master);
    for (const spec of snapshot.mixerChannels) {
      if (spec.id === "mix_master") continue;
      const bus = new MixerChannel(project, spec.id);
      bus.restoreSpec(spec);
      project.mixerChannelList.push(bus);
    }
    const restored = restoreEntities(project, snapshot);
    project.channelList.push(...restored.channels);
    project.sampleList.push(...restored.samples);
    project.trackList.push(...restored.tracks);
    project.automationStore.lanes.push(...restored.automation);
    project.automationStore.clips.push(...restored.automationClips);
    for (const [id, pattern] of restored.patterns) project.patternsById.set(id, pattern);
    for (const id of ids) project.entityIds.add(id);
    project.revisionCounter = BigInt(snapshot.revision);
    return project;
  }

  /** Load editable builders, retaining the asset root for compile, render and save. */
  static async load(directory: string): Promise<Project> {
    const loaded = await loadProject(directory);
    return Project.fromSnapshot(loaded.snapshot, { assetBaseDir: loaded.assetBaseDir });
  }

  /** @internal Preserve an implicit Master until a user explicitly edits it. */
  materializeMaster(): void { this.implicitMaster = false; }
  /** @internal Serialized buses; implicit Master is represented by omission. */
  snapshotMixerChannels(): readonly MixerChannel[] {
    return this.mixerChannelList.filter((bus) => !this.implicitMaster || bus !== this.master);
  }

  /** Save a canonical manifest and content-addressed assets on the control thread. */
  async save(directory: string, options: SaveProjectOptions = {}): Promise<void> {
    await saveProject(this.snapshot(), directory, { assetBaseDir: this.assetBaseDir, ...options });
  }
}
