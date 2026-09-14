//! Project-owned automation builders; mutations remain on the authoring thread.
import { beatToWire, ErrorCode, ID_PREFIXES, OxitoneError } from "@oxitone/protocol";
import { AutomationLane, type AutomationLaneTarget, type AutomationLaneOptions } from "../automation/lane.js";
import { AutomationClip } from "../automation/clip.js";
import type { AutomationSource } from "../automation/source.js";
import { validateLaneTarget } from "../automation/target.js";
import type { Project } from "./project.js";
import type { Track } from "../arrangement/track.js";
export class ProjectAutomation {
  readonly lanes: AutomationLane[] = [];
  readonly clips: AutomationClip[] = [];
  constructor(
    private readonly project: Project,
    private readonly claimId: (prefix: string) => string,
    private readonly entityIds: () => Set<string>,
  ) {}
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
    this.project.assertMutable();
    const instances = target.scope
      ? [
          ...this.project.channels.flatMap((channel) =>
            target.scope === "effectHost"
              ? [...channel.effectInstances]
              : [channel.instrumentInstance, ...channel.effectInstances],
          ),
          ...this.project.mixerChannels.flatMap((bus) => [...bus.effectInstances]),
        ]
      : [];
    validateLaneTarget(
      this.project.id,
      target.scope ? new Set(instances.map((instance) => instance.id)) : this.entityIds(),
      this.lanes,
      target,
      source,
    );
    const lane = new AutomationLane(this.claimId(ID_PREFIXES.automation), target, source, options);
    if (target.entityId === this.project.masterMixerChannelId) this.project.materializeMaster();
    this.lanes.push(lane);
    this.project.touch();
    return lane;
  }

  /** Automation lanes in creation order. */
  get automationLanes(): readonly AutomationLane[] {
    return [...this.lanes];
  }
  get automationClips(): readonly AutomationClip[] {
    return [...this.clips];
  }
  createAutomationClip(lane: AutomationLane, track: Track, startBeat: number, durationBeats?: number): AutomationClip {
    this.project.assertMutable();
    if (!this.lanes.includes(lane) || !this.project.tracks.includes(track)) {
      throw new OxitoneError(ErrorCode.EditTargetMissing, "automation clip lane and track must belong to this project");
    }
    const duration = durationBeats ?? lane.lastBeat ?? lane.loop?.lengthBeats ?? 4;
    if (
      lane.target.entityId === this.project.id ||
      track.tempo !== undefined ||
      !Number.isFinite(startBeat) ||
      startBeat < 0 ||
      !Number.isFinite(duration) ||
      duration <= 0
    ) {
      throw new OxitoneError(
        ErrorCode.InvalidProject,
        "Automation clips require a non-tempo lane, project-time Track and positive duration",
      );
    }
    const clip = new AutomationClip(this.project, {
      id: this.claimId("acl_"),
      laneId: lane.id,
      trackId: track.id,
      startBeat: beatToWire(startBeat),
      durationBeats: beatToWire(duration),
    });
    lane.usePlaylist();
    this.clips.push(clip);
    this.project.touch();
    return clip;
  }
  removeAutomationClip(clip: AutomationClip): void {
    this.project.assertMutable();
    const index = this.clips.indexOf(clip);
    if (index < 0) throw new OxitoneError(ErrorCode.EditTargetMissing, "automation clip belongs to another project");
    this.clips.splice(index, 1);
    this.project.touch();
  }
  removeAutomationLane(lane: AutomationLane): void {
    this.project.assertMutable();
    const index = this.lanes.indexOf(lane);
    if (index < 0)
      throw new OxitoneError(
        ErrorCode.EditTargetMissing,
        "automation lane belongs to another project or has been removed",
      );
    if (this.clips.some((clip) => clip.laneId === lane.id))
      throw new OxitoneError(ErrorCode.InvalidProject, "remove placements before their automation lane");
    this.lanes.splice(index, 1);
    this.project.touch();
  }
}
