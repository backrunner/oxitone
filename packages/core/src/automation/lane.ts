import {
  beatToWire,
  beatFromWire,
  automationLaneSpecSchema,
  ErrorCode,
  OxitoneError,
  type AutomationLaneSpec,
  type EntityId,
  type LoopSpec,
} from "@oxitone/protocol";
import { AutomationSource } from "./source.js";
import { parseAuthoring } from "../authoring-validation.js";

/** Lane target: an automatable entity plus a stable parameter ID. */
export interface AutomationLaneTarget {
  entityId: EntityId;
  parameterId: string;
}

export type AutomationCombine = "replace" | "add" | "multiply" | "max";

/** Authoring loop options for an automation lane. */
export interface AutomationLoopInput {
  startBeat?: number;
  lengthBeats: number;
  count?: number;
  lastBeat?: number;
}

/** Options for `project.addAutomationLane(...)`. */
export interface AutomationLaneOptions {
  combine?: AutomationCombine;
  loop?: AutomationLoopInput;
  lastBeat?: number;
}

function loopToWire(loop: AutomationLoopInput): LoopSpec {
  if (!Number.isFinite(loop.lengthBeats) || loop.lengthBeats <= 0) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `loop lengthBeats must be > 0, got ${loop.lengthBeats}`,
      { details: { path: "automation.loop.lengthBeats" } },
    );
  }
  if (loop.count !== undefined && (!Number.isInteger(loop.count) || loop.count < 1)) {
    throw new OxitoneError(ErrorCode.InvalidProject, `loop count must be an integer >= 1`, {
      details: { path: "automation.loop.count" },
    });
  }
  if (loop.count !== undefined && loop.lastBeat !== undefined) {
    throw new OxitoneError(ErrorCode.InvalidProject, "loop count and lastBeat are mutually exclusive", {
      details: { path: "automation.loop" },
    });
  }
  return {
    ...(loop.startBeat !== undefined ? { startBeat: beatToWire(loop.startBeat) } : {}),
    lengthBeats: beatToWire(loop.lengthBeats),
    ...(loop.count !== undefined ? { count: loop.count } : {}),
    ...(loop.lastBeat !== undefined ? { lastBeat: beatToWire(loop.lastBeat) } : {}),
  };
}

/**
 * A bound automation lane: source + target + combine/loop metadata. Created
 * through `project.addAutomationLane(...)` or `channel.automate(...)`.
 */
export class AutomationLane {
  readonly id: EntityId;
  readonly target: AutomationLaneTarget;
  readonly source: AutomationSource;
  readonly combine?: AutomationCombine;
  readonly loop?: AutomationLoopInput;
  readonly lastBeat?: number;
  private readonly loopSpec?: LoopSpec;
  private restoredSpec?: AutomationLaneSpec;

  /** @internal Use `project.addAutomationLane(...)` instead. */
  constructor(
    id: EntityId,
    target: AutomationLaneTarget,
    source: AutomationSource,
    options: AutomationLaneOptions = {},
  ) {
    this.id = id;
    this.target = Object.freeze({ ...target });
    this.source = source;
    if (options.combine !== undefined) {
      this.combine = options.combine;
    }
    if (options.loop !== undefined) {
      this.loopSpec = loopToWire(options.loop);
      this.loop = Object.freeze({ ...options.loop });
    }
    if (options.lastBeat !== undefined) {
      if (!Number.isFinite(options.lastBeat) || options.lastBeat < 0) {
        throw new OxitoneError(
          ErrorCode.InvalidProject,
          `lane lastBeat must be >= 0, got ${options.lastBeat}`,
          { details: { path: "automation.lastBeat" } },
        );
      }
      this.lastBeat = options.lastBeat;
    }
  }

  /** @internal Retain source trees and rational loop/hold boundaries without evaluation. */
  static fromSpec(input: AutomationLaneSpec): AutomationLane {
    const spec = parseAuthoring(automationLaneSpecSchema, input, "automation");
    const loop = spec.loop;
    const options: AutomationLaneOptions = {};
    if (spec.combine !== undefined) options.combine = spec.combine;
    if (spec.lastBeat !== undefined) options.lastBeat = beatFromWire(spec.lastBeat);
    if (loop !== undefined) {
      options.loop = { lengthBeats: beatFromWire(loop.lengthBeats), ...(loop.startBeat === undefined ? {} : { startBeat: beatFromWire(loop.startBeat) }),
        ...(loop.count === undefined ? {} : { count: loop.count }), ...(loop.lastBeat === undefined ? {} : { lastBeat: beatFromWire(loop.lastBeat) }) };
    }
    const lane = new AutomationLane(spec.id, spec.target, new AutomationSource(spec.source), options);
    lane.restoredSpec = spec;
    return lane;
  }

  /** Wire form for `ProjectSnapshot.automation`. */
  toSpec(): AutomationLaneSpec {
    if (this.restoredSpec !== undefined) return structuredClone(this.restoredSpec);
    return {
      id: this.id,
      target: { entityId: this.target.entityId, parameterId: this.target.parameterId },
      source: this.source.toSpec(),
      ...(this.combine !== undefined ? { combine: this.combine } : {}),
      ...(this.loopSpec !== undefined ? { loop: structuredClone(this.loopSpec) } : {}),
      ...(this.lastBeat !== undefined ? { lastBeat: beatToWire(this.lastBeat) } : {}),
    };
  }
}
