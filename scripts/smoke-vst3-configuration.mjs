import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { configureVst3Plugin, inspectVst3Plugin } from "../packages/vst3/dist/index.js";
import { verifyDenseInstance } from "./vst3-instance-configuration.mjs";
import { verifyLiveDaw } from "./vst3-daw-instance.mjs";

function run(program, args) {
  const result = spawnSync(program, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${program} failed`);
}
if (process.platform !== "darwin") throw new Error("VST3 configuration conformance requires macOS");
run("cargo", [
  "build",
  "--locked",
  "--release",
  "-p",
  "oxitone-vst3-host",
  "--features",
  "host,stream",
  "--bin",
  "oxitone-vst3-host",
  "--example",
  "vst3-control-state-probe",
]);
run("cargo", [
  "build",
  "--release",
  "--locked",
  "--manifest-path",
  "crates/vst3-host/tests/fixtures/transport-plugin/Cargo.toml",
  "--target-dir",
  "target/vst3-transport-fixture",
]);
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-configuration-"));
const host = { hostPath: resolve("target/release/oxitone-vst3-host") };
const before = performance.now();
try {
  const bundlePath = join(root, "Configuration.vst3");
  await mkdir(join(bundlePath, "Contents", "MacOS"), { recursive: true });
  await copyFile(
    "target/vst3-transport-fixture/release/liboxitone_vst3_transport_fixture.dylib",
    join(bundlePath, "Contents", "MacOS", "Configuration"),
  );
  await writeFile(
    join(bundlePath, "Contents", "Info.plist"),
    `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Configuration</string><key>CFBundleIdentifier</key><string>dev.oxitone.configuration-fixture</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>`,
  );
  const source = { bundlePath, classId: "6E33225254224A00AA69301AF3187981", allowPlugins: "any" };
  const initial = await inspectVst3Plugin(source, host);
  assert.equal(initial.audioBuses.outputs.length, 1);
  assert(!initial.parameters.some((p) => p.id === 7));
  const expanded = await configureVst3Plugin(source, { parameters: { 0: 1 / 3 } }, host);
  assert.equal(expanded.audioBuses.outputs.length, 3);
  assert.deepEqual(
    expanded.audioBuses.outputs.map((b) => [b.channels, b.active]),
    [
      [2, true],
      [1, false],
      [2, true],
    ],
  );
  assert(expanded.parameters.some((p) => p.id === 7));
  assert.equal(expanded.parameters.find((p) => p.id === 91).value, 0.5); // 64 frames.
  assert(expanded.parameters.find((p) => p.id === 92).value > 0); // stop/deactivate was required.
  const edited = await configureVst3Plugin(
    source,
    { configuration: expanded.configuration, parameters: { 7: 0.25 } },
    host,
  );
  assert.equal(edited.parameters.find((p) => p.id === 7).value, 0.25);
  assert.equal(edited.parameters.find((p) => p.id === 90).value, 0.25); // processor saw the flush.
  const restored = await configureVst3Plugin(source, { configuration: edited.configuration }, host);
  const liveStart = join(root, "live-start.json");
  await writeFile(
    liveStart,
    JSON.stringify({
      streamProtocolVersion: 11,
      source: { ...source, expectedHash: edited.sha256 },
      options: {
        sampleRate: 48000,
        blockSize: 128,
        tempo: 120,
        timeSignature: [4, 4],
        configuration: edited.configuration,
        parameters: {},
      },
    }),
  );
  assert.equal(restored.parameters.find((p) => p.id === 90).value, 0.25);
  assert.deepEqual(restored.audioBuses, edited.audioBuses);
  const bulk = await configureVst3Plugin(
    source,
    { configuration: edited.configuration, parameters: { 7: 0.75 } },
    host,
  );
  assert.equal(bulk.parameters.find((p) => p.id === 7).value, 0.625);
  assert.equal(bulk.parameters.find((p) => p.id === 90).value, 0.625);
  await assert.rejects(configureVst3Plugin(source, { parameters: { 7: 0.25 } }, host), { code: "PluginConfigInvalid" });
  await assert.rejects(
    configureVst3Plugin(source, { configuration: edited.configuration, parameters: { 90: 0.25 } }, host),
    { code: "PluginConfigInvalid" },
  );
  for (const value of [2 / 3, 1])
    await assert.rejects(configureVst3Plugin(source, { parameters: { 0: value } }, host), {
      code: "PluginCapabilityUnsupported",
    });
  const layouts = [];
  const denseSource = { ...source, classId: "6E33225254224A00AA69301AF3187982" };
  const dense = await configureVst3Plugin(denseSource, {}, host);
  assert.equal(dense.parameters.length, 4096);
  const denseEdited = await configureVst3Plugin(
    denseSource,
    {
      configuration: dense.configuration,
      parameters: { ...dense.configuration.parameters, 4095: 0.25 },
    },
    host,
  );
  const denseRestored = await configureVst3Plugin(
    denseSource,
    {
      configuration: { ...denseEdited.configuration, parameters: {} },
    },
    host,
  );
  assert.equal(denseRestored.parameters.find((p) => p.id === 4095).value, 0.25);
  const denseStart = join(root, "dense-start.json");
  await writeFile(
    denseStart,
    JSON.stringify({
      streamProtocolVersion: 11,
      source: { ...denseSource, expectedHash: dense.sha256 },
      options: {
        sampleRate: 48000,
        blockSize: 128,
        tempo: 120,
        timeSignature: [4, 4],
        configuration: dense.configuration,
        parameters: {},
      },
    }),
  );
  run(resolve("target/release/examples/vst3-control-state-probe"), [host.hostPath, liveStart, denseStart]);
  const denseCaptured = JSON.parse(await readFile(`${denseStart}.captured.json`, "utf8"));
  const denseLiveRestored = await configureVst3Plugin(
    denseSource,
    {
      configuration: { ...denseCaptured.configuration, parameters: {} },
    },
    host,
  );
  assert.equal(denseLiveRestored.parameters.find((p) => p.id === 4095).value, 0.125);
  await verifyDenseInstance(denseSource, dense, host);
  const liveDaw = await verifyLiveDaw(denseSource, dense, host, {
    parameterId: 4095,
    initial: 0.5,
    changed: 0.375,
  });
  for (const suffix of ["797E", "797F", "7980"]) {
    const info = await configureVst3Plugin({ ...source, classId: `6E33225254224A00AA69301AF318${suffix}` }, {}, host);
    layouts.push({ inputs: info.audioBuses.inputs.length, outputs: info.audioBuses.outputs.length });
  }
  assert.deepEqual(layouts, [
    { inputs: 2, outputs: 1 },
    { inputs: 3, outputs: 3 },
    { inputs: 0, outputs: 3 },
  ]);
  const report = resolve(process.env.OXITONE_VST3_CONFIGURATION_REPORT ?? "target/vst3-configuration.json");
  const measured = {
    date: new Date().toISOString(),
    cpu: cpus()[0].model,
    os: `${process.platform} ${release()}`,
    sampleRate: 48000,
    blockSize: 128,
    processedFrames: 0,
    device: null,
    callbackP95: null,
    callbackP99: null,
    xruns: null,
    configurationElapsedMs: performance.now() - before,
    layouts,
    liveDaw,
    checks: [
      "dynamicBusLayout",
      "inactiveMiddle",
      "latencyRestart",
      "parameterTableRestore",
      "processorFlush",
      "stateRoundTrip",
      "bulkControllerValues",
      "dense4096Overrides",
      "unknownParameter",
      "readOnlyParameter",
      "reloadRejected",
      "restartStormBounded",
      "zeroInputInstrument",
      "liveControlBulkValues",
      "liveControlLayoutInvalidation",
      "liveControlReloadInvalidation",
      "liveControlRestartStorm",
      "liveControlDense4096BeforeAudio",
      "sdkGraphDenseInstancesAndStalePromises",
      "dawDenseCaptureUndoRedoSaveReopen",
    ],
  };
  if (process.argv.includes("--benchmark")) {
    const samples = [];
    for (let index = 0; index < 33; index++) {
      const started = performance.now();
      const result = await configureVst3Plugin(source, { configuration: edited.configuration }, host);
      const elapsed = performance.now() - started;
      assert.equal(result.parameters.find((p) => p.id === 90).value, 0.25);
      if (index >= 3) samples.push(elapsed);
    }
    const sorted = samples.toSorted((a, b) => a - b);
    measured.controlBenchmark = {
      kind: "configurationIncludingHelperStartup",
      warmup: 3,
      samplesMs: samples,
      p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1],
      p99Ms: sorted[Math.ceil(sorted.length * 0.99) - 1],
    };
  }
  await writeFile(report, `${JSON.stringify(measured, null, 2)}\n`);
  console.log(`VST3 configuration conformance passed: ${report}`);
} finally {
  await rm(root, { recursive: true, force: true });
}
