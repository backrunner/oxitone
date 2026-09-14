import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import type { Pattern } from "./pattern.js";
import type { Project } from "../project/project.js";

/** Validate the entire composite before claiming any identities. */
export function registerPattern(project: Project, pattern: Pattern, patternsById: Map<string, Pattern>, entityIds: Set<string>): void {
    const pending = [pattern, ...pattern.parts.map(part => part.pattern)];
    const claimedIds = new Set(entityIds);
    const registered = new Map(patternsById);
    for (const part of pattern.parts) {
      if (!project.channels.some(channel => channel.id === part.channelId)) {
        throw new OxitoneError(ErrorCode.InvalidProject, "Pattern part Channel must belong to this project");
      }
    }
    for (const item of pending) {
      const existing = registered.get(item.id);
      if (existing === item) continue;
      const ids = [item.id, ...item.notes.flatMap(note => note.id === undefined ? [] : [note.id])];
      if (existing || ids.some(id => claimedIds.has(id)) || new Set(ids).size !== ids.length) {
        throw new OxitoneError(ErrorCode.InvalidProject, "Pattern or note identity is already registered");
      }
      ids.forEach(id => claimedIds.add(id));
      registered.set(item.id, item);
    }
    for (const id of claimedIds) entityIds.add(id);
    for (const [id, item] of registered) patternsById.set(id, item);

}
