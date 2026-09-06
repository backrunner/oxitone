import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import type { AutomationLane, AutomationLaneTarget } from "./lane.js";
import { AutomationSource } from "./source.js";

/** Authoring ownership and the Project's unique tempo lane; Rust validates parameter descriptors. */
export function validateLaneTarget(projectId: string, entities: ReadonlySet<string>, lanes: readonly AutomationLane[],
  target: AutomationLaneTarget, source: AutomationSource): void {
  if (!(source instanceof AutomationSource)) {
    throw new OxitoneError(ErrorCode.InvalidProject, "lane source must be an AutomationSource", {
      details: { path: "automation.source" },
    });
  }
  if (!entities.has(target.entityId)) {
    throw new OxitoneError(ErrorCode.AutomationTargetInvalid, `unknown automation target entity: ${target.entityId}`, {
      details: { path: "automation.target.entityId" },
    });
  }
  if (target.entityId !== projectId) return;
  if (target.parameterId !== "tempo") {
    throw new OxitoneError(ErrorCode.AutomationTargetInvalid, `project exposes only the 'tempo' parameter, got '${target.parameterId}'`, {
      details: { path: "automation.target.parameterId" },
    });
  }
  if (lanes.some((lane) => lane.target.entityId === projectId)) {
    throw new OxitoneError(ErrorCode.TempoAutomationConflict, "a tempo automation lane already exists for this project", {
      details: { path: "automation.target" },
    });
  }
}
