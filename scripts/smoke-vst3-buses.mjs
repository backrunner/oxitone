// Real native multibus conformance plus Project sidechain rendering. No hardware audio output.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { inspectVst3Plugin, registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";
import { checkOutputRoutes } from "./vst3-output-routes.mjs";
import { checkInsertRoutes } from "./vst3-insert-routes.mjs";

function run(program, args) {
  const result = spawnSync(program, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${program} failed`);
}
if (process.platform !== "darwin") throw new Error("VST3 conformance requires macOS");
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
  "vst3-bus-probe",
]);
run("cargo", [
  "build",
  "--release",
  "--manifest-path",
  "crates/vst3-host/tests/fixtures/transport-plugin/Cargo.toml",
  "--target-dir",
  "target/vst3-transport-fixture",
  "--locked",
]);
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-buses-"));
try {
  const bundlePath = join(root, "Buses.vst3");
  await mkdir(join(bundlePath, "Contents", "MacOS"), { recursive: true });
  await copyFile(
    "target/vst3-transport-fixture/release/liboxitone_vst3_transport_fixture.dylib",
    join(bundlePath, "Contents", "MacOS", "Buses"),
  );
  await writeFile(
    join(bundlePath, "Contents", "Info.plist"),
    `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Buses</string><key>CFBundleIdentifier</key><string>dev.oxitone.bus-fixture</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>`,
  );
  const source = { bundlePath, classId: "6E33225254224A00AA69301AF318797E", allowPlugins: "any" };
  const host = { hostPath: resolve("target/release/oxitone-vst3-host") };
  const start = join(root, "start.json");
  await writeFile(
    start,
    JSON.stringify({
      streamProtocolVersion: 11,
      source,
      options: { sampleRate: 48000, blockSize: 128, parameters: {}, tempo: 120, timeSignature: [4, 4] },
    }),
  );
  const report = resolve(process.env.OXITONE_VST3_BUS_REPORT ?? "target/vst3-buses.json");
  run("target/release/examples/vst3-bus-probe", [host.hostPath, start, report]);
  const measured = JSON.parse(await readFile(report, "utf8"));
  const info = await inspectVst3Plugin(source, host);
  assert.deepEqual(info.audioBuses, {
    inputs: [
      { channels: 2, active: true },
      { channels: 1, active: false },
    ],
    outputs: [{ channels: 2, active: true }],
  });
  const project = new Project({ seed: 271 });
  const plugin = await registerVst3Plugin(project, source, host);
  assert.equal(plugin.kind, "effect");
  const detector = project.addMixerChannel({ masterSendRatio: 0 });
  const target = project.addMixerChannel();
  const channel = project.addChannel({ mixerChannelId: target.id });
  project
    .addTrack()
    .use(channel)
    .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 0.75, velocity: 0.2 }] }))
    .at({ bar: 1 });
  let counter = 0;
  const render = async () => {
    const path = join(root, `graph-${counter++}.wav`);
    await project.renderWav({ path, bitDepth: "float32", dither: "none" });
    return pcm(path);
  };
  const baseline = await render();
  target.inserts = [vst3Config(plugin, { configuration: info.configuration })];
  // A silent detector must silence the product processor, while host bypass preserves main PCM.
  detector.send(target, { sidechain: true });
  const silent = await render();
  assert.ok(
    silent.every((v) => v === 0),
    "silent detector must not alias the main input",
  );
  target.inserts = [{ ...vst3Config(plugin), bypass: true }];
  const bypass = compare(await render(), baseline, 1, "sidechain insert host bypass");
  // Pre-fader send of the same source feeds a second copy to the detector. This produces x*x.
  target.inserts = [];
  channel.mixerChannelId = detector.id;
  detector.send(target, { ratio: 1, preFader: true });
  const direct = await render();
  compare(direct, baseline, 1, "main send baseline");
  // Route the detector's main output to target through a separate channel with identical notes.
  detector.masterSendRatio = 0;
  detector.send(target, { sidechain: true });
  const twin = project.addChannel({ mixerChannelId: target.id });
  project
    .addTrack()
    .use(twin)
    .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 0.75, velocity: 0.2 }] }))
    .at({ bar: 1 });
  target.inserts = [vst3Config(plugin)];
  const product = await render();
  assert.ok(Math.max(...product) > 0.00001, "nonzero sidechain must reach the native processor");
  detector.send(target, { sidechain: true, ratio: 0.5 });
  const detectorGain = compare(await render(), product, 0.5, "sidechain send ratio");
  detector.send(target, { sidechain: true, ratio: 1 });
  // Deterministic reopen/re-registration verifies the same bus activation and graph routing.
  const saved = join(root, "saved");
  await project.save(saved);
  const reopened = await Project.load(saved);
  await registerVst3Plugin(reopened, source, host);
  const path = join(root, "reopened.wav");
  await reopened.renderWav({ path, bitDepth: "float32", dither: "none" });
  const restored = await pcm(path);
  assert.deepEqual(restored, product);
  Object.assign(measured, {
    outputRoutes: await checkOutputRoutes(bundlePath, host, root),
    insertRoutes: await checkInsertRoutes(bundlePath, host, root),
    date: new Date().toISOString(),
    cpu: cpus()[0].model,
    os: `${process.platform} ${release()}`,
    profile: "release",
    compiler: spawnSync("rustc", ["--version", "--verbose"], { encoding: "utf8" }).stdout.trim(),
    graph: {
      silentDetector: true,
      bypass,
      detectorGain,
      nonzeroDetectorPeak: Math.max(...product),
      restoredSamples: restored.length,
    },
  });
  await writeFile(report, `${JSON.stringify(measured, null, 2)}\n`);
  console.log(`VST3 buses and graph sidechain passed: ${report}`);
} finally {
  await rm(root, { recursive: true, force: true });
}
