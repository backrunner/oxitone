import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { basename, join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import { Project } from "@oxitone/core";
import { ErrorCode, PROTOCOL_VERSION } from "@oxitone/protocol";
import { importSample, inspectSample } from "../src/index.js";
import { tempDirectory, wavFile, wavSamples } from "./fixtures.js";

describe("file sample import through Rust", () => {
  it("detects content, returns lossless metadata and leaves the source untouched", () => {
    const directory = tempDirectory();
    const { path, bytes } = wavFile(directory);
    const imported = importSample(relative(process.cwd(), path));
    expect(imported).toEqual({
      assetUri: path, sha256: createHash("sha256").update(bytes).digest("hex"),
      format: "wav", channels: 1, sampleRate: 48_000, frames: 4800n,
      provenance: { sourceChannels: 1, sourceBitDepth: 16, decoder: "oxitone-wav-v1", channelLayoutAction: "kept" },
    });
    expect(inspectSample(path)).toMatchObject({ protocolVersion: PROTOCOL_VERSION, frames: "4800" });
    expect(readFileSync(path)).toEqual(bytes);
    expect(readdirSync(directory)).toEqual([basename(path)]);
  });

  it("imports, prepares with SRC and trim, and renders deterministic audible PCM", async () => {
    const directory = tempDirectory();
    const { path } = wavFile(directory, 1, 24_000);
    const project = new Project({ seed: 71 });
    const sample = project.addSample({ ...importSample(path), edits: { startFrame: "600", endFrame: "1800" } });
    project.addTrack().use(project.addChannel()).sample(sample).at({ bar: 1 }).fitBeats(1);
    const render = async (name: string) => {
      const output = join(directory, name);
      await project.renderWav({ path: output, end: { seconds: 0.1 }, tailSeconds: 0, bitDepth: "float32" });
      return output;
    };
    const first = await render("first.wav");
    const audio = wavSamples(first);
    expect(audio).toHaveLength(4800 * 2);
    expect(audio.every(Number.isFinite)).toBe(true);
    expect(audio.slice(200, 4000).some((value) => Math.abs(value) > 0.01)).toBe(true);
    expect(audio.slice(6000).every((value) => Math.abs(value) < 1e-7)).toBe(true);
    expect(readFileSync(await render("second.wav"))).toEqual(readFileSync(first));
  });

  it("resolves relative assets against a movable base directory", async () => {
    const source = tempDirectory();
    const target = tempDirectory();
    const { path } = wavFile(source, 2);
    const imported = importSample(basename(path), { assetBaseDir: source });
    expect(imported.assetUri).toBe(basename(path));
    expect(importSample(path, { assetBaseDir: source })).toEqual(imported);
    copyFileSync(path, join(target, basename(path)));
    rmSync(path);
    const project = new Project();
    const sample = project.addSample(imported);
    project.addTrack().use(project.addChannel()).sample(sample).at({ bar: 1 });
    const output = join(target, "render.wav");
    await project.renderWav({ path: output, assetBaseDir: target, end: { seconds: 0.1 }, tailSeconds: 0 });
    expect(wavSamples(output).some((value) => Math.abs(value) > 0.01)).toBe(true);
  });

  it("reports source channel provenance separately from prepared stereo dimensions", async () => {
    const directory = tempDirectory();
    const { path } = wavFile(directory, 6);
    const imported = importSample(path);
    expect(imported.channels).toBe(2);
    expect(imported.frames).toBe(4800n);
    expect(imported.provenance).toMatchObject({ sourceChannels: 6, channelLayoutAction: "downmixed-to-stereo" });
    const project = new Project();
    const sample = project.addSample(imported);
    project.addTrack().use(project.addChannel()).sample(sample).at({ bar: 1 });
    expect(project.snapshot().samples[0]).not.toHaveProperty("provenance");
    const session = await project.compile();
    await session.dispose();
  });

  it("rejects changed or removed source bytes during native prepare", async () => {
    const directory = tempDirectory();
    const { path, bytes } = wavFile(directory);
    const project = new Project();
    const sample = project.addSample(importSample(path));
    project.addTrack().use(project.addChannel()).sample(sample).at({ bar: 1 });
    const session = await project.compile();
    await session.dispose();
    bytes[44] = bytes[44]! ^ 0x01;
    writeFileSync(path, bytes);
    await expect(project.compile()).rejects.toMatchObject({ code: ErrorCode.AssetUnavailable, details: { path } });
    rmSync(path);
    await expect(project.compile()).rejects.toMatchObject({ code: ErrorCode.AssetUnavailable });
  });

  it("returns stable errors for missing, corrupt and unsupported files", () => {
    const directory = tempDirectory();
    const path = join(directory, "bad.wav");
    expect(() => importSample(path)).toThrowError(expect.objectContaining({ code: ErrorCode.AssetUnavailable, details: { path } }));
    for (const bytes of [Buffer.from("not audio"), Buffer.from("RIFF\0\0\0\0WAVE")]) {
      writeFileSync(path, bytes);
      expect(() => importSample(path)).toThrowError(expect.objectContaining({ code: ErrorCode.SampleFormatUnsupported, details: { path } }));
    }
  });

  it("validates paths before resolving and rejects assets outside a relative base", () => {
    for (const path of ["", "bad\0path"]) {
      expect(() => importSample(path)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
      expect(() => inspectSample(path)).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
      expect(() => importSample("file.wav", { assetBaseDir: path })).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    }
    expect(() => importSample("../outside.wav", { assetBaseDir: tempDirectory() })).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
  });

  it.skipIf(process.platform !== "darwin")("imports an AAC audio track and retains compressed-source provenance", async () => {
    const directory = tempDirectory();
    const { path } = wavFile(directory, 2);
    const compressed = join(directory, "source.m4a");
    execFileSync("/usr/bin/afconvert", ["-f", "m4af", "-d", "aac", "-b", "128000", path, compressed]);
    const imported = importSample(compressed);
    expect(imported.format).toBe("m4a");
    expect(imported.sha256).toBe(createHash("sha256").update(readFileSync(compressed)).digest("hex"));
    expect(imported.provenance.decoder).toMatch(/^symphonia 0\.5\/aac$/);
    expect(imported.frames).toBeGreaterThanOrEqual(4800n);
    const project = new Project();
    project.addTrack().use(project.addChannel()).sample(project.addSample(imported)).at({ bar: 1 });
    const output = join(directory, "decoded.wav");
    await project.renderWav({ path: output, end: { seconds: 0.2 }, tailSeconds: 0 });
    expect(wavSamples(output).some((value) => Math.abs(value) > 0.01)).toBe(true);
  });
});
