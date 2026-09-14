import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { it, expect, beforeAll, afterAll } from "vitest";
import { createEngine, registerPlugin, dispose, compile, exportMidi } from "oxitone";
import { buildPlugins } from "../src/plugins.js";
import { verifyPlugins } from "../src/verify.js";
import { createDrumSong } from "../src/song.js";
import { readFileSync } from "node:fs";
import { beatToWire } from "@oxitone/protocol";
import { createDubstepSong } from "../src/full/melodic-dubstep.js";
import { midiSnapshot } from "../src/full/midi-export.js";
import { fixturePiano } from "./piano-fixture.js";

const output = mkdtempSync(join(tmpdir(), "oxitone-drums-"));
let plugins: Awaited<ReturnType<typeof buildPlugins>>;
// Cold compilation and Cargo's shared build lock are preparation, not DSP test time.
beforeAll(async () => {
  plugins = await buildPlugins(output);
}, 600_000);
afterAll(() => {
  rmSync(output, { recursive: true, force: true });
});

it("controls real dynamic plugins through both native and Project/Session APIs", async () => {
  const engine = createEngine({ allowPlugins: "any" });
  try {
    for (const options of plugins) registerPlugin(engine, options);
    expect(verifyPlugins(engine, output).diagnostics.every((plugin) => plugin.faults === 0)).toBe(true);
    const project = createDrumSong();
    for (const options of plugins) project.registerPlugin(options, { allowPlugins: "any" });
    const render = { end: { beat: beatToWire(4) }, tailSeconds: 0 };
    await project.renderWav({ ...render, path: join(output, "project.wav") });
    const session = await project.compile();
    try {
      await session.renderWav({ ...render, path: join(output, "session.wav") });
      expect(readFileSync(join(output, "project.wav"))).toEqual(readFileSync(join(output, "session.wav")));
      expect(session.pluginDiagnostics().every((plugin) => plugin.faults === 0)).toBe(true);
      project.registeredPlugins[0]!.libraryPath = "missing";
      expect(project.registeredPlugins[0]!.libraryPath).not.toBe("missing");
      expect(() => project.registerPlugin({ ...plugins[0]!, expectedHash: "0".repeat(64) })).toThrow();
    } finally {
      await session.dispose();
    }
  } finally {
    dispose(engine);
  }
}, 120_000);

it("compiles more than 16 audio tracks without MIDI assignments and checks the limit only on export", () => {
  const engine = createEngine({ allowPlugins: "any" });
  try {
    registerPlugin(engine, plugins[0]!);
    const snapshot = createDubstepSong(fixturePiano(output)).snapshot();
    expect(snapshot.tracks.length).toBeGreaterThan(16);
    expect(snapshot.tracks.every((t) => t.midiChannel === undefined)).toBe(true);
    expect(() => compile(engine, snapshot)).not.toThrow();
    expect(() => exportMidi(engine, snapshot, {})).toThrowError(expect.objectContaining({ code: "MidiChannelLimit" }));
    expect(exportMidi(engine, midiSnapshot(snapshot), {}).bytesBase64).toBeTruthy();
    expect(() => compile(engine, snapshot)).not.toThrow();
  } finally {
    dispose(engine);
  }
}, 30_000);
