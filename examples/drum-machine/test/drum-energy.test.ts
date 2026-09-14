import { expect, it } from "vitest";
import { beatFromWire } from "@oxitone/protocol";
import { createDubstepSong } from "../src/full/melodic-dubstep.js";

it("layers the half-time backbeat and gives builds a denser roll followed by a pickup gap", () => {
  const s = createDubstepSong().snapshot();
  const clips = (name: string) => {
    const track = s.tracks.find((t) => t.name === name)!;
    expect(track, name).toBeTruthy();
    return s.patternClips
      .filter((c) => c.trackId === track.id)
      .map((c) => ({
        bar: beatFromWire(c.startBeat) / 4,
        notes: s.patterns.find((p) => p.id === c.patternId)!.notes,
      }));
  };
  for (const name of [
    "Half-time snare · crack",
    "Snare · body layer",
    "Snare · noise tail layer",
    "Clap · snare layer",
  ]) {
    const drops = clips(name).filter((c) => (c.bar >= 24 && c.bar < 40) || (c.bar >= 72 && c.bar < 88));
    expect(drops).toHaveLength(32);
    expect(drops.every((c) => c.notes.some((n) => beatFromWire(n.start) >= 2 && beatFromWire(n.start) < 2.1))).toBe(
      true,
    );
  }
  const rolls = clips("Build · accelerating snare roll");
  for (const start of [8, 56]) {
    expect([0, 8, 12, 15].map((i) => rolls.find((c) => c.bar === start + i)!.notes.length)).toEqual([4, 8, 16, 24]);
    const last = rolls.find((c) => c.bar === start + 15)!;
    expect(last.notes.every((n) => beatFromWire(n.start) + beatFromWire(n.duration) < 3)).toBe(true);
  }
  expect(clips("Shaker · sixteenth motion")).toHaveLength(32);
  expect(clips("Ride · drop drive").length).toBeGreaterThan(16);
  expect(clips("Toms · phrase fills").length).toBeGreaterThanOrEqual(8);
});

it("keeps piano ornaments on a quieter independent channel and adds complementary chord timbres", () => {
  const s = createDubstepSong().snapshot();
  const keys = s.channels.find((c) => c.name === "Soft Piano · intimate intro")!;
  const ornament = s.channels.find((c) => c.name === "Soft Piano · quiet ornament")!;
  expect(ornament.id).not.toBe(keys.id);
  expect(ornament.level).toBeLessThan(keys.level * 0.6);
  expect(s.tracks.find((t) => t.name === "Soft Piano · horizon theme")!.channelIds).toEqual([ornament.id]);
  for (const name of ["Chords · upper pulse sheen", "Chords · distorted rhythm edge"]) {
    const t = s.tracks.find((t) => t.name === name)!;
    expect(s.patternClips.filter((c) => c.trackId === t.id)).toHaveLength(32);
  }
  expect(s.tracks.length).toBeGreaterThanOrEqual(36);
  expect(s.tracks.every((t) => t.midiChannel === undefined)).toBe(true);
});
