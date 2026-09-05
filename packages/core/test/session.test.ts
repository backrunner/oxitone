import { describe, expect, it, vi } from "vitest";
import { ErrorCode } from "@oxitone/protocol";
import { createAutomationNamespace, Pattern, Project, type TransportPosition } from "../src/index.js";

function projectWithNotes(): Project {
  const project = new Project();
  project.addTrack().use(project.addChannel()).add(new Pattern({ lengthBeats: 4,
    notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }],
  })).at({ bar: 1 });
  return project;
}

describe("Session graph updates and compiled positions", () => {
  it("Project.play refreshes authoring before forwarding the requested position and loop", async () => {
    const project = projectWithNotes();
    const session = await project.compile();
    const play = vi.spyOn(session, "play").mockResolvedValue({ state: "playing", cursor: "0" });
    try {
      project.setTempo(180);
      const loop = { startFrame: 0n, endFrame: 48000n };
      expect(await project.play({ bar: 1, beat: 1 }, loop)).toBe(session);
      expect(session.revision).toBe(BigInt(project.snapshot().revision));
      expect(play).toHaveBeenCalledWith({ bar: 1, beat: 1 }, loop);
      expect((await session.seek({ beat: 3 })).cursor).toBe("48000");
    } finally { play.mockRestore(); await session.dispose(); }
  });
  it("updates on the same engine and preserves compiled exports and positions on rejection", async () => {
    const project = projectWithNotes();
    const session = await project.compile();
    try {
      const engineId = session.engineId;
      const initialMidi = await session.exportMidi({});
      const revision = session.revision;
      project.setTempo(240);
      expect((await session.seek({ beat: 2 })).cursor).toBe("48000");
      expect(await session.exportMidi({})).toEqual(initialMidi);
      expect(await project.exportMidi({})).not.toEqual(initialMidi);
      expect(await session.update()).toBe(session);
      expect(session.engineId).toBe(engineId);
      expect(session.revision).toBeGreaterThan(revision);
      expect((await session.seek({ beat: 2 })).cursor).toBe("24000");
      const accepted = await session.exportMidi({});
      const acceptedRevision = session.revision;
      const bus = project.addMixerChannel();
      const other = project.addMixerChannel();
      bus.send(other);
      other.send(bus);
      await expect(session.update()).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
      expect(session.revision).toBe(acceptedRevision);
      expect((await session.seek({ beat: 2 })).cursor).toBe("24000");
      expect(await session.exportMidi({})).toEqual(accepted);
    } finally { await session.dispose(); }
  });

  it("resolves bar+beat, markers and timecodes against the compiled snapshot and actual engine rate", async () => {
    const project = projectWithNotes();
    project.addTimeSignature({ startBar: 2, numerator: 3, denominator: 4 });
    const marker = project.addMarker("verse", 5);
    const a = createAutomationNamespace();
    project.addAutomationLane({ entityId: project.id, parameterId: "tempo" }, a.constant(Math.log(240 / 20) / Math.log(999 / 20)));
    const session = await project.compile({ sampleRate: 24000 });
    try {
      expect((await session.seek({ bar: 2, beat: 1 })).cursor).toBe("30000");
      expect((await session.seek({ marker: marker.id })).cursor).toBe("30000");
      expect((await session.seek({ seconds: 1.25 })).cursor).toBe("30000");
      expect((await session.seek({ frames: 123n })).cursor).toBe("123");
      expect((await session.seek({ frame: 124 })).cursor).toBe("124");
      project.setTimeSignature(7, 4);
      expect((await session.seek({ bar: 2, beat: 1 })).cursor).toBe("30000");
      await session.update();
      expect((await session.seek({ bar: 2, beat: 1 })).cursor).toBe("48000");
      for (const position of [{ bar: 0 }, { bar: 2, beat: 7 }, { marker: "missing" },
        { seconds: Infinity }, { seconds: -1 }, { seconds: 1e30 }, { frame: -1 },
        { frame: Number.MAX_SAFE_INTEGER + 1 }, { seconds: 1, beat: 2 }, {}]) {
        await expect(session.seek(position as TransportPosition)).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
      }
      expect((await session.seek({ seconds: 0 })).cursor).toBe("0");
    } finally { await session.dispose(); }
  });

  it("clears the active session after disposal and rejects later operations consistently", async () => {
    const project = projectWithNotes();
    const session = await project.compile();
    await session.dispose();
    await session.dispose();
    expect(session.disposed).toBe(true);
    expect(project.session).toBeUndefined();
    for (const run of [() => session.update(), () => session.seek({ beat: 0 }),
      () => session.exportMidi({}), () => session.diagnostics(), () => project.pause()]) {
      await expect(run()).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
    }
    const next = await project.compile();
    try { expect(next.engineId).not.toBe(session.engineId); } finally { await next.dispose(); }
  });
});
