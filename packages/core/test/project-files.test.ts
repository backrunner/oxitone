import { describe, expect, it } from "vitest";
import { mkdtemp, readFile, readdir, rename, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { canonicalEncode, ErrorCode } from "@oxitone/protocol";
import { compile, createEngine, dispose, enqueueTransport, inspectSample, renderWav } from "oxitone";
import { loadProject, Pattern, Project, saveProject } from "../src/index.js";

describe("portable project files", () => {
  it("moves a saved project and reproduces audible native output from relative assets", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-project-"));
    const engine = createEngine();
    try {
      const source = new Project();
      source.addTrack().use(source.addChannel()).add(new Pattern({ lengthBeats: 1,
        notes: [{ pitch: 60, start: 0, duration: 1, velocity: 1 }],
      })).at({ bar: 1 });
      const asset = join(root, "source.wav");
      await source.renderWav({ path: asset, end: { seconds: 0.25 }, tailSeconds: 0 });
      const info = inspectSample(asset);
      const project = new Project({ name: "portable" });
      const sample = project.addSample({ ...info, assetUri: asset, frames: BigInt(info.frames) });
      project.addTrack().use(project.addChannel()).sample(sample).at({ bar: 1 });
      const before = project.snapshot();
      const song = join(root, "song");
      await project.save(song);
      expect(project.snapshot()).toEqual(before);
      const manifest = await readFile(join(song, "oxitone.project.json"), "utf8");
      expect(manifest).toBe(canonicalEncode(JSON.parse(manifest)));
      expect(JSON.parse(manifest)).toMatchObject({ formatVersion: "1.0", projectId: project.id });
      const moved = join(root, "moved song");
      await rename(song, moved);
      await rm(asset);
      const loaded = await loadProject(moved);
      expect(loaded.snapshot.samples[0]?.assetUri).toBe(`assets/${info.sha256}.wav`);
      expect(loaded.snapshot).not.toHaveProperty("formatVersion");
      compile(engine, loaded.snapshot, { assetBaseDir: loaded.assetBaseDir });
      expect(enqueueTransport(engine, { command: "seek", seconds: 0.1 }).cursor).toBe("4800");
      const out = join(root, "render.wav");
      const report = renderWav(engine, loaded.snapshot, { path: out, assetBaseDir: loaded.assetBaseDir, tailSeconds: 0 });
      expect(report.files[0]?.peakDbfs).toBeGreaterThan(-60);
      const audio = await readFile(out);
      await saveProject(loaded.snapshot, moved, { assetBaseDir: moved });
      expect(await readFile(join(moved, "oxitone.project.json"), "utf8")).toBe(manifest);
      const roundtrip = await loadProject(moved);
      renderWav(engine, roundtrip.snapshot, { path: out, assetBaseDir: moved, tailSeconds: 0 });
      expect(await readFile(out)).toEqual(audio);
      const relativeProject = await Project.load(moved);
      expect(relativeProject.assetBaseDir).toBe(moved);
      expect(relativeProject.snapshot()).toEqual(loaded.snapshot);
      await relativeProject.save(moved);
      expect(await readFile(join(moved, "oxitone.project.json"), "utf8")).toBe(manifest);
      await relativeProject.renderWav({ path: out, tailSeconds: 0 });
      expect(await readFile(out)).toEqual(audio);
      const session = await relativeProject.compile();
      try {
        relativeProject.setTempo(90);
        relativeProject.tracks[0]!.sampleClips[0]!.gain = 0.5;
        await session.update();
        expect((await session.renderWav({ path: out, tailSeconds: 0 })).files[0]?.peakDbfs).toBeGreaterThan(-60);
      } finally { await session.dispose(); }
      const copy = join(root, "edited copy");
      await relativeProject.save(copy);
      expect((await Project.load(copy)).snapshot().sampleClips[0]?.gain).toBe(0.5);
      await writeFile(join(moved, loaded.snapshot.samples[0]!.assetUri), "changed");
      await expect(loadProject(moved)).rejects.toMatchObject({ code: ErrorCode.AssetUnavailable });
    } finally { dispose(engine); await rm(root, { recursive: true, force: true }); }
  });

  it("keeps the previous manifest and cleans temporary files when a save fails", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-project-"));
    try {
      const project = new Project();
      await project.save(root);
      const path = join(root, "oxitone.project.json");
      const original = await readFile(path, "utf8");
      const source = join(root, "changed.wav");
      await writeFile(source, "unexpected content");
      project.addSample({ assetUri: source, sha256: "00".repeat(32), format: "wav", sampleRate: 48000, channels: 1, frames: 1 });
      await expect(project.save(root)).rejects.toMatchObject({ code: ErrorCode.AssetUnavailable });
      expect(await readFile(path, "utf8")).toBe(original);
      expect(await readdir(join(root, "assets"))).toEqual([]);
      expect((await readdir(root)).some((name) => name.endsWith(".tmp"))).toBe(false);
    } finally { await rm(root, { recursive: true, force: true }); }
  });

  it("rejects incompatible versions, malformed JSON, traversal and escaping symlinks", async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-project-"));
    try {
      const song = join(root, "song");
      await new Project().save(song);
      const path = join(song, "oxitone.project.json");
      const original = JSON.parse(await readFile(path, "utf8"));
      for (const patch of [{ formatVersion: "2.0" }, { protocolVersion: "2.0" }]) {
        await writeFile(path, JSON.stringify({ ...original, ...patch }));
        await expect(loadProject(song)).rejects.toMatchObject({ code: ErrorCode.ProtocolVersionUnsupported });
      }
      await writeFile(path, "{");
      await expect(loadProject(song)).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
      const outside = join(root, "outside.wav");
      await writeFile(outside, "test");
      await symlink(outside, join(song, "assets", "link.wav"));
      for (const assetUri of ["../outside.wav", outside, "assets/link.wav"]) {
        await writeFile(path, JSON.stringify({ ...original, samples: [{ id: "smp_x", assetUri,
          sha256: "00".repeat(32), format: "wav", sampleRate: 48000, channels: 1, frames: "1" }] }));
        await expect(loadProject(song)).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
      }
    } finally { await rm(root, { recursive: true, force: true }); }
  });
});
