import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { Pattern, Project, grandPiano, softPiano } from "@oxitone/core";
import { createEngine, dispose, renderWav } from "oxitone";
import { beatToWire } from "@oxitone/protocol";
import { fixturePiano } from "./piano-fixture.js";

it("renders soft and grand recorded velocity layers through Rust without a device or downloads", () => {
  const dir = mkdtempSync(join(tmpdir(), "oxitone-piano-")), engine = createEngine();
  try {
    const levels = [softPiano, grandPiano].map((instrument, index) => {
      const p = new Project(), bank = fixturePiano(dir)(p);
      p.addTrack("Piano").use(p.addChannel({ instrument: instrument(bank) })).add(new Pattern({
        lengthBeats: 2, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }],
      })).at({ bar: 1 });
      return renderWav(engine, p.snapshot(), { path: join(dir, `${index}.wav`),
        end: { beat: beatToWire(2) }, tailSeconds: 0 }).files[0]!.peakDbfs;
    });
    expect(levels.every(Number.isFinite)).toBe(true);
    expect(levels[1]! - levels[0]!).toBeGreaterThan(10);
  } finally { dispose(engine); rmSync(dir, { recursive: true, force: true }); }
});
