import { describe, expect, it } from "vitest";
import { Pattern, Project, createAutomationNamespace } from "../src/index.js";

describe("Playlist arrangement transactions", () => {
  it("places and moves reusable Patterns without changing Channel membership", () => {
    const project = new Project();
    const first = project.addTrack("Intro");
    const second = project.addTrack("Drop");
    const channel = project.addChannel({ name: "Lead" });
    first.use(channel);
    const pattern = new Pattern({ lengthBeats: 4, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.8 }] });
    first.pattern(pattern).at({ bar: 1 });
    project.arrange({ action: "place", kind: "pattern", resource: 0, track: 1, startBeat: 8, durationBeats: 4 });
    expect(second.clips).toHaveLength(1);
    expect(second.clips[0]?.pattern.id).toBe(pattern.id);
    project.arrange({ action: "move", kind: "pattern", resource: 0, clip: 0, track: 1, startBeat: 12 });
    expect(first.clips).toHaveLength(0);
    expect(second.clips).toHaveLength(2);
    expect(second.clips.some(clip => clip.startBeat === 12)).toBe(true);
    expect(second.channelIds).toEqual([channel.id]);
  });

  it("places, moves and restores automation clips as reusable lane placements", () => {
    const project = new Project();
    const track = project.addTrack("Filter");
    const lane = project.addAutomationLane({ entityId: project.addChannel().id, parameterId: "level" }, createAutomationNamespace().constant(0.5));
    project.arrange({ action: "place", kind: "automation", resource: 0, track: 0, startBeat: 4, durationBeats: 8 });
    expect(project.automationClips[0]?.laneId).toBe(lane.id);
    expect(project.snapshot().automationClips?.[0]?.trackId).toBe(track.id);
    project.arrange({ action: "move", kind: "automation", resource: 0, clip: 0, track: 0, startBeat: 12 });
    expect(project.automationClips[0]?.startBeat).toBe(12);
    const restored = Project.fromSnapshot(project.snapshot());
    expect(restored.automationClips[0]?.startBeat).toBe(12);
  });
});
