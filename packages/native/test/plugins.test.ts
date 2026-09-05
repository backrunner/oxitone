import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";
import { beatToWire, pluginManifestSchema, type ProjectSnapshot } from "@oxitone/protocol";
import { compile, createEngine, dispose, getPluginDiagnostics, registerPlugin, renderWav } from "../src/index.js";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const dir = mkdtempSync(join(tmpdir(), "oxitone-plugins-"));
const fixture = join(root, "crates/render/tests/fixtures");
const manifest = pluginManifestSchema.parse(JSON.parse(readFileSync(join(fixture, "gain.json"), "utf8")));

function build(name: string, extra: string[] = []): string {
  const path = join(dir, `${name}${process.platform === "darwin" ? ".dylib" : ".so"}`);
  execFileSync("cc", ["-std=c11", "-shared", "-fPIC", "-O2", "-I", join(root, "include"),
    ...extra, join(fixture, "gain.c"), "-o", path]);
  return path;
}

function snapshot(gain?: number): ProjectSnapshot {
  return {
    protocolVersion: "1.0", revision: "1", id: "prj_plugin", sampleRate: 48000, blockSize: 128, seed: 42,
    tempoMap: [{ startBeat: beatToWire(0), bpm: 120 }],
    timeSignatureMap: [{ startBar: 1, numerator: 4, denominator: 4 }], markers: [],
    tracks: [{ id: "trk_plugin", channelIds: ["chn_plugin"], patternClipIds: ["pcl_plugin"], sampleClipIds: [] }],
    patterns: [{ id: "pat_plugin", lengthBeats: beatToWire(1), notes: [
      { pitch: 64, start: beatToWire(0), duration: beatToWire(1), velocity: 0.5 },
    ] }],
    patternClips: [{ id: "pcl_plugin", patternId: "pat_plugin", trackId: "trk_plugin",
      startBeat: beatToWire(0), durationBeats: beatToWire(1) }],
    sampleClips: [], samples: [], mixerChannels: [], automation: [],
    channels: [{ id: "chn_plugin", instrument: { pluginId: "oxitone.wavetable", pluginVersion: "1.0.0", parameters: {} },
      effectChain: gain === undefined ? [] : [{ pluginId: "fixture.gain", pluginVersion: "1.0.0", parameters: { gain } }],
      level: 0.05, pan: 0, mixerChannelId: "mix_master" }],
  };
}

afterAll(() => rmSync(dir, { recursive: true, force: true }));

describe("dynamic plugins through the native facade", () => {
  it("registers per engine, compiles and renders the C effect with deterministic gain", () => {
    const libraryPath = build("gain");
    const expectedHash = createHash("sha256").update(readFileSync(libraryPath)).digest("hex");
    const engine = createEngine({ allowPlugins: "any" });
    const other = createEngine({ allowPlugins: "any" });
    try {
      const options = { libraryPath, expectedHash, manifest };
      const registered = registerPlugin(engine, options);
      expect(registered.sha256).toBe(expectedHash);
      expect(registerPlugin(engine, options)).toEqual(registered);
      expect(() => compile(other, snapshot(1))).toThrow();
      expect(() => registerPlugin(engine, { ...options, expectedHash: "0".repeat(64) })).toThrow();
      compile(engine, snapshot(1));
      const plain = join(dir, "plain.wav");
      const unity = join(dir, "unity.wav");
      const half = join(dir, "half.wav");
      const dry = renderWav(engine, snapshot(), { path: plain });
      renderWav(engine, snapshot(1), { path: unity });
      expect(readFileSync(unity).equals(readFileSync(plain))).toBe(true);
      const wet = renderWav(engine, snapshot(0.5), { path: half });
      expect(dry.files[0]!.peakDbfs).toBeGreaterThan(-60);
      expect(dry.files[0]!.peakDbfs - wet.files[0]!.peakDbfs).toBeCloseTo(6.0206, 3);
      expect(getPluginDiagnostics(engine)).toEqual([{ pluginId: "fixture.gain", pluginVersion: "1.0.0", faults: 0 }]);
      const changedBinary = build("different", ["-DFIXTURE_FAULT=2"]);
      expect(() => registerPlugin(engine, { libraryPath: changedBinary, manifest })).toThrow();
    } finally {
      dispose(engine);
      dispose(other);
    }
  });

  it("mutes a non-finite plugin and exposes its fault count", () => {
    const engine = createEngine({ allowPlugins: "any" });
    try {
      registerPlugin(engine, { libraryPath: build("nan", ["-DFIXTURE_FAULT=2"]), manifest });
      const report = renderWav(engine, snapshot(1), { path: join(dir, "muted.wav") });
      expect(report.files[0]!.peakDbfs).toBe(-144);
      expect(getPluginDiagnostics(engine)[0]!.faults).toBe(1);
    } finally {
      dispose(engine);
    }
  });
});
