// Installed Vesti MIDI Synth -> VestiGain. Offline PCM and simulated output only.
import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { inspectVst3Plugin, registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";

const gainPath = process.env.OXITONE_VST3_FIXTURE;
const synthPath = process.env.OXITONE_VST3_INSTRUMENT;
assert(gainPath && synthPath, "Set OXITONE_VST3_FIXTURE (VestiGain) and OXITONE_VST3_INSTRUMENT (VestiMIDISynth)");
const host = { hostPath: resolve(process.env.OXITONE_VST3_HOST_PATH ?? "target/release/oxitone-vst3-host") };
const source = (bundlePath, classId) => ({ bundlePath: resolve(bundlePath), classId, allowPlugins: "any" });
const synthSource = source(synthPath, "564553544953594e5448303030303031");
const gainSource = source(gainPath, "56455354494741494e30303030303031");
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-instrument-"));
const project = new Project({ seed: 117 });
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const results = {};
let counter = 0;
async function render(owner = project) {
  const path = join(root, `render-${counter++}.wav`);
  await owner.renderWav({ path, bitDepth: "float32", dither: "none" });
  return pcm(path);
}
async function active(session) {
  const end = performance.now() + 5000;
  while (performance.now() < end) {
    const inventory = session.vst3Instances();
    if (inventory.state === "active") return inventory;
    await pause(5);
  }
  throw new Error("instrument graph was not published");
}
try {
  const synthInfo = await inspectVst3Plugin(synthSource, host);
  const gainInfo = await inspectVst3Plugin(gainSource, host);
  const synth = await registerVst3Plugin(project, synthSource, host);
  const gain = await registerVst3Plugin(project, gainSource, host);
  assert.equal(synth.kind, "instrument");
  assert.equal(gain.kind, "effect");
  const channel = project.addChannel({ instrument: vst3Config(synth, { configuration: synthInfo.configuration }) });
  project
    .addTrack()
    .use(channel)
    .add(
      new Pattern({
        lengthBeats: 2,
        notes: [
          { pitch: 60, start: 0, duration: 0.5, velocity: 0.2 },
          { pitch: 67, start: 0, duration: 0.5, velocity: 0.15 },
          { pitch: 72, start: 0.75, duration: 0.25, velocity: 0.1 },
        ],
      }),
    )
    .at({ bar: 1 });
  const baseline = await render();
  const peak = (samples) => samples.reduce((value, sample) => Math.max(value, Math.abs(sample)), 0);
  assert(peak(baseline.slice(2048, 12000)) > 0.001, "authored notes must produce sound");
  assert(peak(baseline.slice(-12000)) < 1e-6, "note-off must release voices");
  channel.effectChain = [vst3Config(gain, { configuration: gainInfo.configuration, parameters: { 0: 0.75, 1: 0 } })];
  const processed = await render();
  results.instrumentAndEffect = compare(processed, baseline, 10 ** (-6 / 20), "synth through -6 dB effect");

  const session = await project.compile({ audioBackend: "simulated", renderAheadBlocks: 16 });
  let inventory = await active(session);
  const instrumentId = channel.instrument.instanceId;
  const effectId = channel.effectChain[0].instanceId;
  assert.deepEqual(
    inventory.instances.map((instance) => instance.instanceId),
    [instrumentId, effectId].toSorted(),
  );
  const target = (instanceId) => ({ graphGeneration: inventory.graphGeneration, instanceId });
  const capture = (id) => session.controlVst3Instance(target(id), { kind: "capture" });
  const authored = project.snapshot();
  await session.controlVst3Instance(target(instrumentId), { kind: "setParameter", parameterId: 0, value: 0.2 });
  await session.play({ bar: 1 }, { startFrame: 0, endFrame: 12000 });
  await pause(350);
  const captured = (await capture(instrumentId)).state.info.configuration;
  // This vendor stores its normalized parameter as float32.
  assert.equal(captured.parameters["0"], Math.fround(0.2), "first block must preserve preplay edits");
  assert.equal((await capture(effectId)).state.info.configuration.parameters["0"], 0.75);
  assert.deepEqual(project.snapshot(), authored, "live control cannot edit the source");
  results.beforeUpdate = await session.diagnostics();
  await session.pause();
  const stale = target(instrumentId);
  channel.instrument = { ...channel.instrument, parameters: captured.parameters, state: captured };
  await session.update();
  inventory = await active(session);
  await assert.rejects(session.controlVst3Instance(stale, { kind: "poll" }), { code: "PluginTaskConflict" });
  assert.equal((await capture(instrumentId)).state.info.configuration.parameters["0"], Math.fround(0.2));
  const accepted = await render();
  assert(peak(accepted) < peak(processed), "accepted synth level must affect exported PCM");
  const saved = join(root, "saved");
  await project.save(saved);
  const restored = await Project.load(saved);
  await registerVst3Plugin(restored, synthSource, host);
  await registerVst3Plugin(restored, gainSource, host);
  results.savedState = compare(await render(restored), accepted, 1, "saved instrument and effect state");

  await session.play({ bar: 1 }, { startFrame: 0, endFrame: 12000 });
  await pause(350);
  await session.seek({ bar: 1 });
  await pause(350);
  await session.pause();
  await pause(20);
  assert.equal((await session.diagnostics()).state, "paused");
  await session.play();
  await pause(350);
  results.afterUpdate = await session.diagnostics();
  await session.stop();
  const deadline = performance.now() + 1000;
  do {
    results.stopped = await session.diagnostics();
    if (results.stopped.state === "stopped") break;
    await pause(5);
  } while (performance.now() < deadline);
  assert.equal(results.stopped.state, "stopped");
  results.faults = session.pluginDiagnostics();
  assert(results.faults.every((plugin) => plugin.faults === 0));
  results.checks = [
    "noteOnOff",
    "polyphony",
    "effectPcm",
    "independentParameters",
    "preplayEdit",
    "liveCapture",
    "stateUpdate",
    "saveReopen",
    "loopSeekPauseResumeStop",
  ];
  const report = resolve(process.env.OXITONE_VST3_INSTRUMENT_REPORT ?? "target/vst3-instrument.json");
  await mkdir(dirname(report), { recursive: true });
  await writeFile(
    report,
    `${JSON.stringify(
      {
        date: new Date().toISOString(),
        cpu: cpus()[0].model,
        os: `${process.platform} ${release()}`,
        profile: "release",
        sampleRate: 48000,
        blockSize: 128,
        device: "simulated stereo sink",
        callbackP95Ms: null,
        callbackP99Ms: null,
        plugins: [synthInfo, gainInfo].map(({ name, version, sha256, classId }) => ({
          name,
          version,
          sha256,
          classId,
        })),
        results,
      },
      null,
      2,
    )}\n`,
  );
  for (const [phase, diagnostics] of Object.entries({
    beforeUpdate: results.beforeUpdate,
    afterUpdate: results.afterUpdate,
    stopped: results.stopped,
  })) {
    assert(diagnostics.blocks > 0, `${phase}: no processed blocks`);
    // Worker timing stays in the report; an over-budget block is not a ring underrun.
    for (const key of ["xruns", "nanBlocks", "queueDrops"])
      assert.equal(diagnostics[key], 0, `${phase}: ${key}; see ${report}`);
  }
  console.log(`VST3 installed instrument/effect conformance passed: ${report}`);
} finally {
  await project.session?.dispose();
  await rm(root, { recursive: true, force: true });
}
