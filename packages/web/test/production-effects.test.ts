import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { convolver, effect, Pattern, Project, wavetable } from "@oxitone/core";
import { effectPluginIds, type EffectKind } from "@oxitone/protocol";
import { WasmEngine } from "../src/wasm.js";

it("runs the production palette and custom IR on import-free Wasm with native PCM parity", async () => {
  const directory = await mkdtemp(join(tmpdir(), "oxitone-effects-"));
  const module = await WebAssembly.compile(await readFile(new URL("../dist/oxitone.wasm", import.meta.url)));
  expect(WebAssembly.Module.imports(module)).toEqual([]);
  const engine = await WasmEngine.create(module);
  try {
    const project = new Project({ seed: 73 });
    for (const [i, kind] of (Object.keys(effectPluginIds) as EffectKind[]).entries()) {
      const channel = project.addChannel({ instrument: wavetable({ oscA: { wave: i % 2 ? "saw" : "sine" }, level: 0.06 }),
        effectChain: [effect(kind, {}, { mix: 0.7 })] });
      project.addTrack().use(channel).add(new Pattern({ lengthBeats: 4, notes: [{ pitch: 48 + i, start: 0, duration: 3, velocity: 0.4 }] })).at({ bar: 1 });
    }
    project.master.addEffect(effect("limiter", { ceilingDb: -1 }));
    engine.compile(project);
    // Engine-generated PCM WAV supplies a small deterministic, hash-verified IR to both hosts.
    const impulse = engine.renderWav({ frames: 4096, bitDepth: 32 });
    const impulseView = new DataView(impulse.buffer, impulse.byteOffset);
    let impulsePeak = 0;
    for (let i = 44; i < impulse.length; i += 4) impulsePeak = Math.max(impulsePeak, Math.abs(impulseView.getFloat32(i, true)));
    expect(impulsePeak).toBeGreaterThan(0.001);
    const info = engine.importSample(impulse, "wav");
    const impulsePath = join(directory, "impulse.wav");
    await writeFile(impulsePath, impulse);
    const sample = project.addSample({ ...info, frames: BigInt(info.frames), assetUri: impulsePath });
    project.channels[0]!.addEffect(convolver(sample.id, { outputDb: -12 }, { mix: 0.2 }));
    engine.compile(project);
    engine.transport({ command: "play" });
    const before = engine.memoryDiagnostics();
    let peak = 0;
    for (let i = 0; i < 128; i++) {
      for (const channel of engine.process()) for (const value of channel) {
        expect(Number.isFinite(value)).toBe(true); peak = Math.max(peak, Math.abs(value));
      }
    }
    expect(peak).toBeGreaterThan(0.005);
    const after = engine.memoryDiagnostics();
    expect(after).toEqual(before);
    const wasm = engine.renderWav({ frames: 16384, bitDepth: 32 });
    const path = join(directory, "native.wav");
    await project.renderWav({ path, end: { frames: "16384" }, tailSeconds: 0, bitDepth: "float32", dither: "none" });
    const native = await readFile(path);
    expect(wasm.length).toBe(native.length);
    const a = new DataView(wasm.buffer, wasm.byteOffset), b = new DataView(native.buffer, native.byteOffset);
    let difference = 0;
    for (let i = 44; i < wasm.length; i += 4) difference = Math.max(difference, Math.abs(a.getFloat32(i, true) - b.getFloat32(i, true)));
    expect(difference).toBeLessThan(0.0002);
    await writeFile(new URL("../../../target/effects-wasm-parity.json", import.meta.url), JSON.stringify({
      effects: Object.values(effectPluginIds), customImpulse: true, sampleRate: 48000, blockSize: 128,
      processBlocks: 128, comparedFrames: 16384, maxPcmDifference: difference, tolerance: 0.0002,
      processMemoryBefore: before, processMemoryAfter: after, peak, impulsePeak,
      imports: WebAssembly.Module.imports(module), outputDevice: null,
    }, null, 2) + "\n");
  } finally { engine.dispose(); await rm(directory, { recursive: true, force: true }); }
}, 180000);
