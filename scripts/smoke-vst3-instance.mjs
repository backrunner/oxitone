// Actual SDK -> async N-API -> live VestiGain graph. Simulated sink only.
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile, mkdir } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { inspectVst3Plugin, registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { pcm, compare } from "./vst3-project-audio.mjs";

const bundlePath = process.env.OXITONE_VST3_FIXTURE;
if (!bundlePath) throw new Error("Set OXITONE_VST3_FIXTURE to VestiGain.vst3");
const source = { bundlePath: resolve(bundlePath), classId: "56455354494741494e30303030303031", allowPlugins: "any" };
const host = { hostPath: resolve(process.env.OXITONE_VST3_HOST_PATH ?? "target/release/oxitone-vst3-host") };
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-instance-"));
const project = new Project({ seed: 271 });
const channel = project.addChannel();
project
  .addTrack()
  .use(channel)
  .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 0.5, velocity: 0.2 }] }))
  .at({ bar: 1 });
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function active(session) {
  const end = performance.now() + 5000;
  while (performance.now() < end) {
    const inventory = session.vst3Instances();
    if (inventory.state === "active") return inventory;
    await pause(5);
  }
  throw new Error("graph control publication timed out");
}
const results = {};
try {
  const metadata = await inspectVst3Plugin(source, host);
  const plugin = await registerVst3Plugin(project, source, host);
  const config = (value) =>
    vst3Config(plugin, { configuration: metadata.configuration, parameters: { 0: value, 1: 0 } });
  channel.effectChain = [config(0.75), config(0.875)];
  project.master.inserts = [config(0.8125)];
  const session = await project.compile({ audioBackend: "simulated", renderAheadBlocks: 16 });
  let inventory = await active(session);
  const original = project.snapshot();
  const [first, second] = channel.effectChain;
  const master = project.master.inserts[0];
  const ids = [first.instanceId, second.instanceId, master.instanceId];
  assert.deepEqual(
    inventory.instances.map((i) => i.instanceId),
    ids.toSorted(),
  );
  const target = (id, generation = inventory.graphGeneration) => ({ graphGeneration: generation, instanceId: id });
  const control = (id, command) => session.controlVst3Instance(target(id), command);
  const capture = async (id, value) => {
    const result = await control(id, { kind: "capture" });
    assert.equal(result.state.info.configuration.parameters["0"], value);
    return result.state.info.configuration;
  };
  await session.renderWav({ path: join(root, "source.wav"), bitDepth: "float32", dither: "none" });
  await control(first.instanceId, { kind: "setParameter", parameterId: 0, value: 0.25 });
  await capture(first.instanceId, 0.25);
  await capture(second.instanceId, 0.875);
  await capture(master.instanceId, 0.8125);
  assert.deepEqual(project.snapshot(), original, "audition must not mutate authoring data");
  await session.renderWav({ path: join(root, "audition.wav"), bitDepth: "float32", dither: "none" });
  assert.deepEqual(await readFile(join(root, "source.wav")), await readFile(join(root, "audition.wav")));
  assert.equal(session.vst3Instances().graphGeneration, inventory.graphGeneration);
  await assert.rejects(control(first.instanceId, { kind: "setParameter", parameterId: 9999, value: 0.5 }), {
    code: "PluginConfigInvalid",
  });
  await session.play({ bar: 1 }, { startFrame: 0, endFrame: 12000 });
  await pause(150);
  await capture(first.instanceId, 0.25); // First graph block must not replay the old 0.75 seed.
  const samples = [];
  for (let index = 0; index < 33; index++) {
    const value = index % 2 ? 0.25 : 0.5;
    const start = performance.now();
    await control(first.instanceId, { kind: "setParameter", parameterId: 0, value });
    await capture(first.instanceId, value);
    if (index >= 3) samples.push(performance.now() - start);
    await capture(second.instanceId, 0.875);
    await pause(10);
  }
  const captured = await capture(first.instanceId, 0.5);
  results.playing = await session.diagnostics();
  await session.pause();
  // Publish an explicit source edit using captured processor state, then reorder the same IDs.
  channel.effectChain = [second, { ...first, parameters: captured.parameters, state: captured }];
  const stale = target(first.instanceId);
  const pending = session.controlVst3Instance(stale, { kind: "capture" }).then(
    () => null,
    (error) => error,
  );
  await session.update();
  inventory = await active(session);
  assert.equal((await pending)?.code, "PluginTaskConflict", "late Promise must not accept a retired graph");
  await assert.rejects(session.controlVst3Instance(stale, { kind: "poll" }), { code: "PluginTaskConflict" });
  await capture(first.instanceId, 0.5);
  await capture(second.instanceId, 0.875);
  await session.renderWav({ path: join(root, "accepted.wav"), bitDepth: "float32", dither: "none" });
  results.acceptedPcm = compare(
    await pcm(join(root, "accepted.wav")),
    await pcm(join(root, "source.wav")),
    10 ** (-18 / 20),
    "captured state accepted into source",
  );
  const accepted = channel.effectChain;
  channel.effectChain = [{ ...accepted[0], parameters: { ...accepted[0].parameters, 9999: 0.5 } }, accepted[1]];
  await assert.rejects(session.update());
  assert.equal(session.vst3Instances().graphGeneration, inventory.graphGeneration);
  await capture(first.instanceId, 0.5);
  channel.effectChain = accepted;
  const deleted = target(first.instanceId);
  channel.effectChain = [second];
  await session.update();
  inventory = await active(session);
  assert(!inventory.instances.some((i) => i.instanceId === first.instanceId));
  await assert.rejects(session.controlVst3Instance(deleted, { kind: "poll" }), { code: "PluginTaskConflict" });
  await assert.rejects(control(first.instanceId, { kind: "poll" }), { code: "PluginConfigInvalid" });
  const faults = session.pluginDiagnostics();
  assert(faults.every((p) => p.faults === 0));
  const sorted = samples.toSorted((a, b) => a - b);
  results.control = {
    operation: "SDK setParameter plus capture during simulated playback",
    warmup: 3,
    samplesMs: samples,
    p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1],
    p99Ms: sorted[Math.ceil(sorted.length * 0.99) - 1],
  };
  results.checks = [
    "threeIndependentInstances",
    "preplayEditSurvivesFirstBlock",
    "liveCapture",
    "sourceUnchangedByAudition",
    "offlineUsesSource",
    "capturedStateAccepted",
    "reorderByStableId",
    "stalePromiseRejected",
    "failedCompilePreservesTarget",
    "deletedInstanceRejected",
  ];
  const report = resolve(process.env.OXITONE_VST3_INSTANCE_REPORT ?? "target/vst3-instance.json");
  await mkdir(dirname(report), { recursive: true });
  await writeFile(
    report,
    `${JSON.stringify({ date: new Date().toISOString(), cpu: cpus()[0].model, os: `${process.platform} ${release()}`, profile: "release", sampleRate: 48000, blockSize: 128, device: "simulated stereo sink", callbackP95: null, callbackP99: null, results }, null, 2)}\n`,
  );
  // Worker timing stays in the report; an over-budget block is not a ring underrun.
  for (const key of ["xruns", "nanBlocks", "queueDrops"])
    assert.equal(results.playing[key], 0, `${key}: see ${report}`);
  console.log(`VST3 live SDK instance conformance passed: ${report}`);
} finally {
  await project.session?.dispose();
  await rm(root, { recursive: true, force: true });
}
