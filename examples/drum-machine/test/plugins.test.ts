import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { it, expect } from "vitest";
import { createEngine, registerPlugin, dispose } from "oxitone";
import { buildPlugins } from "../src/plugins.js";
import { verifyPlugins } from "../src/verify.js";
import { createDrumSong } from "../src/song.js";
import { readFileSync } from "node:fs";
import { beatToWire } from "@oxitone/protocol";

it("controls real dynamic plugins through both native and Project/Session APIs", async () => {
  const output = mkdtempSync(join(tmpdir(), "oxitone-drums-"));
  const engine = createEngine({ allowPlugins: "any" });
  try {
    const plugins = buildPlugins(output);
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
    } finally { await session.dispose(); }
  } finally {
    dispose(engine);
    rmSync(output, { recursive: true, force: true });
  }
}, 120_000);
