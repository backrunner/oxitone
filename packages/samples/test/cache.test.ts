import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, renameSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { describe, expect, it } from "vitest";
import { Project } from "@oxitone/core";
import { ErrorCode } from "@oxitone/protocol";
import { compile, createEngine, dispose } from "@oxitone/native";
import { cacheSample, importSample } from "../src/index.js";
import { tempDirectory, wavFile, wavSamples } from "./fixtures.js";

describe("normalized sample cache", () => {
  it("publishes deterministic float32 PCM, reuses immutable files and detects changed sources", () => {
    const directory = tempDirectory();
    const { path, bytes } = wavFile(directory, 6, 24_000);
    const options = { cacheDir: "assets", assetBaseDir: directory };
    const imported = importSample(basename(path), options);
    expect(imported).toMatchObject({
      format: "wav",
      sampleRate: 24_000,
      channels: 2,
      frames: 2400n,
      assetUri: `assets/${imported.sha256}.wav`,
      provenance: {
        sourceChannels: 6,
        sourceFormat: "wav",
        sourceSampleRate: 24_000,
        cacheEncoding: "wav-f32-v1",
        channelLayoutAction: "downmixed-to-stereo",
        sourceSha256: createHash("sha256").update(bytes).digest("hex"),
      },
    });
    const cachedPath = join(directory, imported.assetUri);
    const cached = readFileSync(cachedPath);
    expect(createHash("sha256").update(cached).digest("hex")).toBe(imported.sha256);
    expect(cached.readUInt16LE(20)).toBe(3);
    expect(cached.readUInt16LE(34)).toBe(32);
    expect(wavSamples(cachedPath).some((value) => Math.abs(value) > 1)).toBe(true); // no peak normalization/clipping
    const mtime = statSync(cachedPath).mtimeMs;
    expect(importSample(path, options)).toEqual(imported);
    expect(statSync(cachedPath).mtimeMs).toBe(mtime);
    expect(readFileSync(path)).toEqual(bytes);
    bytes.writeInt16LE(1234, 44);
    writeFileSync(path, bytes);
    const changed = importSample(path, options);
    expect(changed.sha256).not.toBe(imported.sha256);
    expect(readdirSync(join(directory, "assets"))).toHaveLength(2);
    expect(readFileSync(cachedPath)).toEqual(cached);
  });

  it.skipIf(process.platform !== "darwin")(
    "caches AAC and survives source deletion, project relocation and native round trips",
    async () => {
      const source = tempDirectory();
      const base = tempDirectory();
      const { path } = wavFile(source, 2);
      const compressed = join(source, "compressed.m4a");
      execFileSync("/usr/bin/afconvert", ["-f", "m4af", "-d", "aac", "-b", "128000", path, compressed]);
      const original = importSample(compressed);
      const imported = importSample(compressed, { assetBaseDir: base, cacheDir: "assets" });
      expect(imported.format).toBe("wav");
      expect(imported.provenance).toMatchObject({ ...original.provenance, cacheEncoding: "wav-f32-v1" });
      expect(imported.frames).toBe(original.frames);
      const makeProject = (sample: typeof imported) => {
        const project = new Project({ seed: 21 });
        const asset = project.addSample({ ...sample, edits: { startFrame: "100", endFrame: "4000" } });
        project.addTrack().use(project.addChannel()).sample(asset).at({ bar: 1 });
        return { project, asset };
      };
      const { project, asset } = makeProject(imported);
      const { project: raw } = makeProject(original);
      const render = async (project: Project, name: string, assetBaseDir?: string) => {
        const output = join(source, name);
        await project.renderWav({
          path: output,
          ...(assetBaseDir === undefined ? {} : { assetBaseDir }),
          end: { seconds: 0.15 },
          tailSeconds: 0,
          bitDepth: "float32",
        });
        return readFileSync(output);
      };
      const expected = await render(raw, "raw.wav");
      expect(await render(project, "cached.wav", base)).toEqual(expected);
      rmSync(compressed);
      const published = join(base, "project");
      await project.save(published, { assetBaseDir: base });
      const relocated = join(tempDirectory(), "moved");
      renameSync(published, relocated);
      rmSync(base, { recursive: true });
      const restored = await Project.load(relocated);
      expect(restored.snapshot().samples[0]?.provenance).toEqual(imported.provenance);
      const engine = createEngine();
      try {
        expect(compile(engine, restored.snapshot(), { assetBaseDir: relocated }).samples[0]?.provenance).toEqual(
          imported.provenance,
        );
      } finally {
        dispose(engine);
      }
      expect(await render(restored, "restored.wav")).toEqual(expected);
      const copy = asset.provenance!;
      copy.decoder = "mutated";
      expect(asset.provenance?.decoder).toBe(imported.provenance.decoder);
      const next = join(source, "saved-again");
      await restored.save(next);
      expect(readFileSync(join(next, "oxitone.project.json"))).toEqual(
        readFileSync(join(relocated, "oxitone.project.json")),
      );
      const invalid = structuredClone(restored.snapshot());
      invalid.samples[0]!.provenance!.sourceSha256 = "wrong";
      expect(() => Project.fromSnapshot(invalid)).toThrowError(
        expect.objectContaining({ code: ErrorCode.InvalidProject }),
      );
    },
  );

  it("rejects corrupt or linked cache assets and invalid destinations without overwriting", () => {
    const directory = tempDirectory();
    const { path, bytes } = wavFile(directory);
    const cacheDir = join(directory, "assets");
    const cached = cacheSample(path, cacheDir);
    const wav = readFileSync(cached.path);
    writeFileSync(cached.path, "corrupt");
    expect(() => cacheSample(path, cacheDir)).toThrowError(
      expect.objectContaining({ code: ErrorCode.AssetUnavailable, details: { path: cached.path } }),
    );
    expect(readFileSync(cached.path, "utf8")).toBe("corrupt");
    rmSync(cached.path);
    const target = join(directory, "target.wav");
    writeFileSync(target, wav);
    symlinkSync(target, cached.path);
    expect(() => cacheSample(path, cacheDir)).toThrowError(
      expect.objectContaining({ code: ErrorCode.AssetUnavailable }),
    );
    expect(readFileSync(target)).toEqual(wav);
    expect(readFileSync(path)).toEqual(bytes);
    expect(readdirSync(cacheDir)).toEqual([basename(cached.path)]);
    for (const value of ["", "bad\0path"]) {
      expect(() => cacheSample(path, value)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
      expect(() => importSample(path, { cacheDir: value })).toThrowError(
        expect.objectContaining({ code: ErrorCode.InvalidProject }),
      );
    }
    expect(() => importSample(path, { assetBaseDir: directory, cacheDir: "../escape-cache" })).toThrowError(
      expect.objectContaining({ code: ErrorCode.InvalidProject }),
    );
    expect(() => cacheSample(path, path)).toThrowError(expect.objectContaining({ code: ErrorCode.AssetUnavailable }));
  });
});
