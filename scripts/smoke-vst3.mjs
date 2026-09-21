// Explicit local VestiGain regression. All PCM is offline; no output device is created.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { inspectVst3Plugin, renderVst3Wav, loadVst3Preset, saveVst3Preset } from "../packages/vst3/dist/index.js";

const bundlePath = process.env.OXITONE_VST3_FIXTURE;
if (!bundlePath) throw new Error("Set OXITONE_VST3_FIXTURE to an installed VestiGain.vst3 bundle");
const source = { bundlePath: resolve(bundlePath), classId: "56455354494741494e30303030303031", allowPlugins: "any" };
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-smoke-"));
const host = { hostPath: resolve(process.env.OXITONE_VST3_HOST_PATH ?? "target/release/oxitone-vst3-host") };

function monoWave(frames) {
  const bytes = Buffer.alloc(44 + frames * 4);
  bytes.write("RIFF");
  bytes.writeUInt32LE(bytes.length - 8, 4);
  bytes.write("WAVEfmt ", 8);
  bytes.writeUInt32LE(16, 16);
  bytes.writeUInt16LE(3, 20);
  bytes.writeUInt16LE(1, 22);
  bytes.writeUInt32LE(48000, 24);
  bytes.writeUInt32LE(192000, 28);
  bytes.writeUInt16LE(4, 32);
  bytes.writeUInt16LE(32, 34);
  bytes.write("data", 36);
  bytes.writeUInt32LE(frames * 4, 40);
  for (let frame = 0; frame < frames; frame++) bytes.writeFloatLE(0.25, 44 + frame * 4);
  return bytes;
}
async function samples(path, frames) {
  const bytes = await readFile(path);
  assert.equal(bytes.toString("ascii", 0, 4), "RIFF");
  let data;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const size = bytes.readUInt32LE(offset + 4);
    if (bytes.toString("ascii", offset, offset + 4) === "fmt ") {
      assert.equal(bytes.readUInt16LE(offset + 10), 2);
      assert.equal(bytes.readUInt32LE(offset + 12), 48000);
      assert.equal(bytes.readUInt16LE(offset + 22), 32);
    }
    if (bytes.toString("ascii", offset, offset + 4) === "data") data = bytes.subarray(offset + 8, offset + 8 + size);
    offset += 8 + size + (size % 2);
  }
  assert.equal(data?.length, frames * 8);
  return Array.from({ length: frames * 2 }, (_, index) => data.readFloatLE(index * 4));
}
function timings(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const at = (p) => sorted[Math.ceil(sorted.length * p) - 1];
  return { p50Ms: at(0.5), p95Ms: at(0.95), p99Ms: at(0.99), samplesMs: values };
}

try {
  const info = await inspectVst3Plugin(source, host);
  assert.equal(info.classId, source.classId.toLowerCase());
  assert.equal(info.name, "Vesti Gain");
  assert.equal(info.noteInput, false);
  assert.ok(info.configuration);
  assert.ok(info.parameters.some((p) => p.id === 1 && p.stepCount === 1));
  const inputPath = join(root, "mono.wav");
  await writeFile(inputPath, monoWave(48000));
  const options = {
    path: join(root, "out.wav"),
    frames: 129,
    tailFrames: 7,
    inputPath,
    parameters: { 1: 1 },
    configuration: info.configuration,
  };
  const pinned = { ...source, expectedHash: info.sha256 };
  const preset = {
    formatVersion: 1,
    kind: "oxitone-vst3-preset",
    name: info.name,
    source: { bundlePath: source.bundlePath, classId: source.classId },
    configuration: { ...info.configuration, parameters: { ...info.configuration.parameters, 1: 1 } },
  };
  const presetPath = join(root, "sdk-preset.json");
  await saveVst3Preset(presetPath, preset);
  const loadedPreset = await loadVst3Preset(presetPath);
  assert.deepEqual(loadedPreset, preset);
  options.configuration = loadedPreset.configuration;
  const report = await renderVst3Wav(pinned, options, host);
  assert.equal(report.frames, 136);
  assert.equal(report.pluginSha256, info.sha256);
  const pcm = await samples(options.path, 136);
  for (let i = 0; i < pcm.length; i++)
    assert.ok(Math.abs(pcm[i] - (i < 258 ? 0.25 : 0)) < 1e-6, `PCM sample ${i}: ${pcm[i]}`);
  const before = await readFile(options.path);
  await assert.rejects(renderVst3Wav(pinned, options, host), { code: "AssetUnavailable" });
  assert.deepEqual(await readFile(options.path), before);
  await assert.rejects(inspectVst3Plugin({ ...source, expectedHash: "0".repeat(64) }, host), {
    code: "PluginManifestMismatch",
  });
  await assert.rejects(inspectVst3Plugin({ ...source, classId: "0".repeat(32) }, host));
  const alias = join(root, "Alias.vst3");
  await symlink(source.bundlePath, alias);
  await assert.rejects(inspectVst3Plugin({ ...source, bundlePath: alias }, host));
  await assert.rejects(
    renderVst3Wav(
      pinned,
      {
        ...options,
        path: join(root, "wrong-state.wav"),
        configuration: { ...info.configuration, sha256: "0".repeat(64) },
      },
      host,
    ),
    { code: "PluginManifestMismatch" },
  );
  await assert.rejects(
    renderVst3Wav(pinned, { ...options, path: join(root, "unknown.wav"), parameters: { 9999: 0.5 } }, host),
  );
  console.log(
    "VST3 native smoke passed: exact class, state restore, mono duplication, 129-frame content + 7 silent tail frames, create-only WAV, hash/state/parameter/symlink rejection; no device",
  );

  if (process.argv.includes("--benchmark")) {
    const inspect = [],
      render = [],
      savePreset = [],
      loadPreset = [];
    for (let i = -3; i < 20; i++) {
      let start = performance.now();
      await inspectVst3Plugin(pinned, host);
      if (i >= 0) inspect.push(performance.now() - start);
      start = performance.now();
      await renderVst3Wav(
        pinned,
        { ...options, path: join(root, `bench-${i}.wav`), frames: 48000, tailFrames: 0 },
        host,
      );
      if (i >= 0) render.push(performance.now() - start);
      start = performance.now();
      await saveVst3Preset(join(root, `preset-${i}.json`), preset);
      if (i >= 0) savePreset.push(performance.now() - start);
      start = performance.now();
      await loadVst3Preset(join(root, `preset-${i}.json`));
      if (i >= 0) loadPreset.push(performance.now() - start);
    }
    const result = {
      scenario: "vst3-offline-helper",
      date: new Date().toISOString(),
      cpu: cpus()[0].model,
      os: `${process.platform} ${release()}`,
      rust: execFileSync("rustc", ["-vV"], { encoding: "utf8" }).trim(),
      profile: "release",
      node: process.version,
      plugin: { name: info.name, version: info.version, sha256: info.sha256 },
      sampleRate: 48000,
      blockSize: 128,
      channels: 2,
      voices: 0,
      seed: null,
      input: "mono constant 0.25",
      parameters: { 1: 1 },
      frames: 48000,
      warmup: 3,
      iterations: 20,
      preset: {
        formatVersion: 1,
        opaqueStateBytes: Buffer.from(preset.configuration.stateBase64, "base64").length,
        parameterCount: Object.keys(preset.configuration.parameters).length,
      },
      inspect: timings(inspect),
      render: timings(render),
      savePreset: timings(savePreset),
      loadPreset: timings(loadPreset),
      device: null,
      callbackP95Ms: null,
      callbackP99Ms: null,
      cpuUtilization: null,
      xruns: null,
      scope:
        "helper timings include bundle hashing, load, offline process, WAV I/O and teardown; preset timings include bounded data validation and create-only file publication with fsync; warm filesystem cache; no realtime claims",
    };
    const path = resolve(process.env.OXITONE_VST3_BENCH_OUTPUT ?? "target/vst3-benchmark.json");
    await writeFile(path, `${JSON.stringify(result, null, 2)}\n`);
    console.log(
      `VST3 benchmark: inspect p95=${result.inspect.p95Ms.toFixed(2)}ms; 1s render p95=${result.render.p95Ms.toFixed(2)}ms; ${path}`,
    );
  }
  for (const mode of ["stream", "schedule", "managed"]) {
    if (!process.argv.includes(`--${mode}`)) continue;
    const startPath = join(root, "stream-start.json");
    await writeFile(
      startPath,
      JSON.stringify({
        streamProtocolVersion: 11,
        source: pinned,
        options: {
          sampleRate: 48000,
          blockSize: 128,
          configuration: loadedPreset.configuration,
          parameters: { 1: 1 },
          tempo: 120,
          timeSignature: [4, 4],
        },
      }),
    );
    const variable = `OXITONE_VST3_${mode.toUpperCase()}`;
    const reportPath = resolve(process.env[`${variable}_REPORT`] ?? `target/vst3-${mode}.json`);
    const rawReportPath = join(root, `${mode}-report.json`);
    try {
      execFileSync(
        resolve(process.env[`${variable}_PROBE`] ?? `target/release/examples/vst3-${mode}-probe`),
        [host.hostPath, startPath, rawReportPath],
        { stdio: "inherit", timeout: 30000 },
      );
    } finally {
      // Preserve environment metadata for failed probes too; never relabel an older report.
      const bytes = await readFile(rawReportPath, "utf8").catch((error) => {
        if (error.code === "ENOENT") return null;
        throw error;
      });
      if (bytes !== null) {
        const report = JSON.parse(bytes);
        Object.assign(report, {
          date: new Date().toISOString(),
          cpu: cpus()[0].model,
          os: `${process.platform} ${release()}`,
        });
        await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
      }
    }
  }
  if (process.argv.includes("--gui")) {
    execFileSync(process.execPath, ["scripts/smoke-daw.mjs"], {
      stdio: "inherit",
      env: {
        ...process.env,
        OXITONE_VST3_HOST_PATH: host.hostPath,
        OXITONE_PREVIEW_CAPTURE_VST3: "1",
        OXITONE_PREVIEW_CAPTURE_CONFIGURATION: "1",
        OXITONE_VST3_FIXTURE: source.bundlePath,
        OXITONE_VST3_INPUT: inputPath,
        OXITONE_VST3_OUTPUT: join(root, "gui.wav"),
        OXITONE_VST3_OUTPUT_2: join(root, "gui-save.wav"),
        OXITONE_VST3_OUTPUT_3: join(root, "gui-loaded.wav"),
        OXITONE_VST3_PRESET: join(root, "gui-preset.json"),
        OXITONE_PREVIEW_CAPTURE_OUTPUT: resolve("target/daw-vst3.png"),
      },
    });
    const gui = await samples(join(root, "gui.wav"), 52800);
    assert.ok(gui.slice(0, 96000).every((value) => Math.abs(value - 0.25) < 1e-6));
    assert.ok(gui.slice(96000).every((value) => value === 0));
    assert.deepEqual(await samples(join(root, "gui-loaded.wav"), 52800), gui);
    assert.equal((await loadVst3Preset(join(root, "gui-preset.json"))).configuration.parameters["1"], 1);
    console.log("VST3 GUI PCM matches the bypassed mono input and silent tail");
  }
} finally {
  await rm(root, { recursive: true, force: true });
}
