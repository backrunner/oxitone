// End-to-end MIDI output: actual VST3 processors, native port, graph, persistence and simulated sink.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";
import { checkMidiDocument } from "./vst3-midi-document.mjs";
import { checkMidiOnly } from "./vst3-midi-only.mjs";
import { checkSysEx } from "./vst3-sysex.mjs";

function run(program, args) {
  const result = spawnSync(program, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${program} failed`);
}
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
  "vst3-midi-probe",
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
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-midi-"));
try {
  const bundlePath = join(root, "Midi.vst3");
  await mkdir(join(bundlePath, "Contents", "MacOS"), { recursive: true });
  await copyFile(
    "target/vst3-transport-fixture/release/liboxitone_vst3_transport_fixture.dylib",
    join(bundlePath, "Contents", "MacOS", "Midi"),
  );
  await writeFile(
    join(bundlePath, "Contents", "Info.plist"),
    `<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Midi</string><key>CFBundleIdentifier</key><string>dev.oxitone.midi-fixture</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>`,
  );
  const source = (suffix) => ({ bundlePath, classId: `6E33225254224A00AA69301AF318${suffix}`, allowPlugins: "any" });
  const host = { hostPath: resolve("target/release/oxitone-vst3-host") };
  const start = join(root, "start.json");
  await writeFile(
    start,
    JSON.stringify({
      streamProtocolVersion: 11,
      source: source("7986"),
      options: { sampleRate: 48000, blockSize: 128, parameters: {}, tempo: 120, timeSignature: [4, 4] },
    }),
  );
  const probe = join(root, "probe.json");
  run("target/release/examples/vst3-midi-probe", [host.hostPath, start, probe]);
  const report = JSON.parse(await readFile(probe, "utf8"));
  const project = new Project({ seed: 129 });
  const normal = await registerVst3Plugin(project, source("7980"), host);
  const emitter = await registerVst3Plugin(project, source("7986"), host);
  const fx = await registerVst3Plugin(project, source("7987"), host);
  const [target, origin] = [project.addChannel(), project.addChannel()].sort((a, b) => a.id.localeCompare(b.id));
  origin.instrument = vst3Config(emitter);
  origin.mute = true;
  target.instrument = vst3Config(normal);
  const notes = new Pattern({
    lengthBeats: 1,
    notes: [
      { pitch: 60, start: 13 / 24000, duration: 96 / 24000, velocity: 64 / 127 },
      { pitch: 64, start: 0.25, duration: 0.25, velocity: 87 / 127 },
    ],
  });
  project.addTrack().use(origin).add(notes).at({ bar: 1 });
  const reference = new Project({ seed: 129 });
  await registerVst3Plugin(reference, source("7980"), host);
  const refTarget = reference.addChannel({ instrument: vst3Config(normal) });
  reference.addTrack().use(refTarget).add(notes).at({ bar: 1 });
  let serial = 0;
  const render = async (p) => {
    const path = join(root, `midi-${serial++}.wav`);
    await p.renderWav({ path, end: { seconds: 0.5 }, tailSeconds: 0, bitDepth: "float32", dither: "none" });
    return pcm(path);
  };
  const baseline = await render(reference);
  assert.ok(baseline.some((v) => v !== 0));
  assert.ok((await render(project)).every((v) => v === 0));
  origin.routeMidi(origin.instrumentInstance, [target]);
  const checks = [compare(await render(project), baseline, 1, "reverse-ID MIDI dependency preserves sample timing")];
  const second = project.addChannel({ instrument: vst3Config(normal) });
  origin.routeMidi(origin.instrumentInstance, [target, second]);
  checks.push(compare(await render(project), baseline, 2, "MIDI fan-out delivers once per destination"));
  second.mute = true;
  checks.push(compare(await render(project), baseline, 1, "target audio mute preserves event processing"));
  const saved = join(root, "saved-midi");
  await project.save(saved);
  const reopened = await Project.load(saved);
  for (const suffix of ["7980", "7986", "7987"]) await registerVst3Plugin(reopened, source(suffix), host);
  checks.push(compare(await render(reopened), baseline, 1, "MIDI routes survive project save/load"));
  origin.routeMidi(origin.instrumentInstance, []);
  const insert = origin.addEffect({ ...vst3Config(fx), mix: 0, bypass: true });
  origin.routeMidi(insert, [target]);
  const generated = await render(project);
  assert.ok(
    generated.some((v) => v !== 0),
    "muted bypassed insert still emits note-offs and notes",
  );
  origin.reorderEffects([insert]);
  assert.deepEqual(await render(project), generated);
  const session = await project.compile({ audioBackend: "simulated", latencyMode: "direct", renderAheadBlocks: 16 });
  try {
    await session.play({ bar: 1 }, { startFrame: 0, endFrame: 6000 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    await session.seek({ bar: 1 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    await session.stop();
    report.simulated = await session.diagnostics();
    assert.ok(report.simulated.blocks > 100);
    // Worker timing stays in the report; an over-budget block is not a ring underrun.
    for (const key of ["xruns", "nanBlocks", "queueDrops"]) assert.equal(report.simulated[key], 0, key);
    assert.ok(session.pluginDiagnostics().every((p) => p.faults === 0));
  } finally {
    await session.dispose();
  }
  origin.removeEffect(insert);
  assert.deepEqual(origin.midiRoutes, {});
  assert.ok((await render(project)).every((v) => v === 0));
  target.routeMidi(target.instrumentInstance, [origin]);
  await assert.rejects(project.compile({ audioBackend: "simulated" }), (e) => e.code === "PluginCapabilityUnsupported");
  Object.assign(report, {
    checks,
    insertMidi: true,
    rejectedMissingCapability: true,
    date: new Date().toISOString(),
    cpu: cpus()[0].model,
    os: `${process.platform} ${release()}`,
  });
  report.sourceTransactions = await checkMidiDocument(project.registeredVst3Plugins, root);
  report.midiOnly = await checkMidiOnly(source, host, root);
  report.sysexGraph = await checkSysEx(source, host, root);
  const output = resolve(process.env.OXITONE_VST3_MIDI_REPORT ?? "target/vst3-midi.json");
  await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`VST3 MIDI integration passed: ${output}`);
} finally {
  await rm(root, { recursive: true, force: true });
}
