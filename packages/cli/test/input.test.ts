import { execFile } from "node:child_process";
import { mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { describe, expect, it } from "vitest";
import { encodeProjectSnapshot } from "@oxitone/protocol";
import { Pattern, Project } from "@oxitone/core";
import { inspectSample } from "@oxitone/native";

const cli = fileURLToPath(new URL("../dist/index.js", import.meta.url));
const execute = promisify(execFile);
async function run(cwd: string, ...args: string[]) {
  return JSON.parse((await execute(process.execPath, [cli, ...args], { cwd, encoding: "utf8" })).stdout);
}
function rejection(...args: string[]) {
  return execute(process.execPath, [cli, ...args], { encoding: "utf8" }).then(
    (result) => ({ status: 0, stderr: result.stderr }),
    (error: { code?: string | number; stderr: string }) => ({ status: error.code, stderr: error.stderr }),
  );
}

describe("CLI project inputs", () => {
  it("renders moved project directories/manifests and relative snapshots from another cwd", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-cli-"));
    try {
      const synth = new Project();
      synth
        .addTrack()
        .use(synth.addChannel())
        .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }] }))
        .at({ bar: 1 });
      const source = join(root, "source.wav");
      await synth.renderWav({ path: source, end: { seconds: 0.1 }, tailSeconds: 0 });
      const info = inspectSample(source);
      const project = new Project();
      const sample = project.addSample({ ...info, assetUri: source, frames: BigInt(info.frames) });
      project.addTrack().use(project.addChannel()).sample(sample).at({ bar: 1 });
      await project.save(join(root, "original"));
      await rename(join(root, "original"), join(root, "moved"));
      await rm(source);
      const report = await run(root, "render", "moved", "directory.wav");
      expect(report.files[0].peakDbfs).toBeGreaterThan(-60);
      await run(root, "render", "moved/oxitone.project.json", "manifest.wav");
      const restored = await Project.load(join(root, "moved"));
      await writeFile(join(root, "moved", "snapshot.json"), encodeProjectSnapshot(restored.snapshot()));
      await run(root, "render", "moved/snapshot.json", "snapshot.wav");
      expect(await readFile(join(root, "directory.wav"))).toEqual(await readFile(join(root, "manifest.wav")));
      expect(await readFile(join(root, "directory.wav"))).toEqual(await readFile(join(root, "snapshot.wav")));
      // MIDI project files use the same loader; native export still owns SMF bytes.
      await synth.save(join(root, "midi-project"));
      await run(root, "export-midi", "midi-project", "cli.mid");
      await synth.exportMidi({ path: join(root, "sdk.mid") });
      expect(await readFile(join(root, "cli.mid"))).toEqual(await readFile(join(root, "sdk.mid")));
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }, 60_000); // Several real child-process renders; speed is measured by dedicated benchmarks.

  it("reports structured load errors and preserves output on rejection", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-cli-invalid-"));
    try {
      const project = new Project();
      await project.save(root);
      const path = join(root, "oxitone.project.json");
      const manifest = JSON.parse(await readFile(path, "utf8"));
      await writeFile(path, JSON.stringify({ ...manifest, formatVersion: "99.0" }));
      const output = join(root, "existing.wav");
      await writeFile(output, "keep");
      const rejected = await rejection("render", root, output);
      expect(rejected.status).toBe(1);
      expect(JSON.parse(rejected.stderr)).toMatchObject({ code: "ProtocolVersionUnsupported" });
      expect(await readFile(output, "utf8")).toBe("keep");
      const missing = await rejection("render", join(root, "missing"), output);
      expect(missing.status).toBe(1);
      expect(JSON.parse(missing.stderr)).toMatchObject({ code: "AssetUnavailable" });
      const malformed = join(root, "bad.json");
      await writeFile(malformed, "{broken");
      const invalid = await rejection("render", malformed, output);
      expect(invalid.status).toBe(1);
      expect(JSON.parse(invalid.stderr)).toMatchObject({ code: "InvalidProject" });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
