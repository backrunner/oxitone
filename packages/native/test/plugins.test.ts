import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { beforeAll, afterAll, describe, expect, it } from "vitest";
import { beatToWire, pluginManifestSchema, type ProjectSnapshot } from "@oxitone/protocol";
import { compile, createEngine, dispose, getPluginDiagnostics, registerPlugin, renderWav, setParameter } from "../src/index.js";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const dir = mkdtempSync(join(tmpdir(), "oxitone-plugins-"));
const fixture = join(root, "crates/render/tests/fixtures");
const manifest = pluginManifestSchema.parse(JSON.parse(readFileSync(join(fixture, "gain.json"), "utf8")));

const execute = promisify(execFile);
async function build(name: string, extra: string[] = []): Promise<string> {
  const path = join(dir, `${name}${process.platform === "darwin" ? ".dylib" : ".so"}`);
  await execute("cc", ["-std=c11", "-shared", "-fPIC", "-O2", "-I", join(root, "include"),
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

let gainLibrary: string, faultLibrary: string;
// Compiler startup belongs to fixture preparation, and must not block Vitest's RPC loop.
beforeAll(async () => {
  gainLibrary = await build("gain");
  faultLibrary = await build("fault", ["-DFIXTURE_FAULT=2"]);
}, 600_000);
afterAll(() => rmSync(dir, { recursive: true, force: true }));

describe("dynamic plugins through the native facade", () => {
  it("applies insert automation and queued host parameters to dynamic effects", () => {
    const engine = createEngine({ allowPlugins: "any" });
    try {
      registerPlugin(engine, { libraryPath: gainLibrary, manifest });
      for (const owner of ["chn_plugin", "mix_master"]) {
        const input = snapshot(1);
        if (owner === "mix_master") {
          input.mixerChannels.push({ id: owner, level: 1, balance: 0,
            inserts: input.channels[0]!.effectChain, sends: [] });
          input.channels[0]!.effectChain = [];
        }
        compile(engine, input);
        const reference = renderWav(engine, input, { path: join(dir, `${owner}-reference.wav`) });
        setParameter(engine, owner, "insert.0.parameter.gain", 0.5);
        const host = renderWav(engine, input, { path: join(dir, `${owner}-host.wav`) });
        expect(reference.files[0]!.peakDbfs - host.files[0]!.peakDbfs).toBeCloseTo(6.0206, 3);
        input.automation.push({ id: "auto_gain", target: { entityId: owner, parameterId: "insert.0.parameter.gain" },
          source: { kind: "constant", value: 0.25 } }); // descriptor gain is 0..2; physical = 0.5
        compile(engine, input); // Clear host events so this render exercises the lane alone.
        const automated = renderWav(engine, input, { path: join(dir, `${owner}-automation.wav`) });
        expect(readFileSync(automated.files[0]!.path)).toEqual(readFileSync(host.files[0]!.path));
      }
      expect(getPluginDiagnostics(engine)[0]!.faults).toBe(0);
    } finally {
      dispose(engine);
    }
  }, 60_000);

  it("registers per engine, compiles and renders the C effect with deterministic gain", () => {
    const libraryPath = gainLibrary;
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
      expect(() => registerPlugin(engine, { libraryPath: faultLibrary, manifest })).toThrow();
    } finally {
      dispose(engine);
      dispose(other);
    }
  }, 60_000);

  it("mutes a non-finite plugin and exposes its fault count", () => {
    const engine = createEngine({ allowPlugins: "any" });
    try {
      registerPlugin(engine, { libraryPath: faultLibrary, manifest });
      const report = renderWav(engine, snapshot(1), { path: join(dir, "muted.wav") });
      expect(report.files[0]!.peakDbfs).toBe(-144);
      expect(getPluginDiagnostics(engine)[0]!.faults).toBe(1);
    } finally {
      dispose(engine);
    }
  }, 60_000);
});
