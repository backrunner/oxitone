// Real local VestiGain through Project, native registration and the complete audio graph.
// Playback uses ONLY the simulated native sink; never opens a hardware audio output.
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, mkdir } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { inspectVst3Plugin, registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";

const bundlePath = process.env.OXITONE_VST3_FIXTURE;
if (!bundlePath) throw new Error("Set OXITONE_VST3_FIXTURE to the local VestiGain.vst3 bundle");
const source = { bundlePath: resolve(bundlePath), classId: "56455354494741494e30303030303031", allowPlugins: "any" };
const host = { hostPath: resolve(process.env.OXITONE_VST3_HOST_PATH ?? "target/release/oxitone-vst3-host") };
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-project-"));
const results = {};
const project = new Project({ seed: 271 });
const channel = project.addChannel();
project
  .addTrack()
  .use(channel)
  .add(
    new Pattern({
      lengthBeats: 1,
      notes: [
        { pitch: 60, start: 0, duration: 0.5, velocity: 0.2 },
        { pitch: 67, start: 0.5, duration: 0.25, velocity: 0.1 },
      ],
    }),
  )
  .at({ bar: 1 });
let counter = 0;
async function render(owner = project) {
  const path = join(root, `render-${counter++}.wav`);
  const report = await owner.renderWav({ path, bitDepth: "float32", dither: "none" });
  return { report, pcm: await pcm(path) };
}
try {
  const baseline = await render();
  const metadata = await inspectVst3Plugin(source, host);
  const plugin = await registerVst3Plugin(project, source, host);
  assert.equal(plugin.kind, "effect");
  const config = (parameters) => vst3Config(plugin, { configuration: metadata.configuration, parameters });
  channel.effectChain = [config({ 1: 1 })];
  const bypass = await render();
  results.pluginBypass = compare(bypass.pcm, baseline.pcm, 1, "plugin bypass");
  assert.equal(bypass.report.graphLatencyFrames, baseline.report.graphLatencyFrames);
  const gain = 10 ** (-6 / 20);
  channel.effectChain = [config({ 0: 0.75, 1: 0 })];
  results.gain = compare((await render()).pcm, baseline.pcm, gain, "-6 dB gain");
  channel.effectChain = [{ ...config({ 0: 0.75, 1: 0 }), mix: 0.25 }];
  results.mix = compare((await render()).pcm, baseline.pcm, 0.75 + 0.25 * gain, "insert mix");
  channel.effectChain = [{ ...config({ 0: 0.75, 1: 0 }), bypass: true }];
  results.hostBypass = compare((await render()).pcm, baseline.pcm, 1, "host bypass");
  channel.effectChain = [config({ 0: 0.75, 1: 0 }), config({ 0: 0.75, 1: 0 })];
  results.serial = compare((await render()).pcm, baseline.pcm, gain * gain, "serial inserts");
  channel.effectChain = [];
  project.master.inserts = [config({ 0: 0.75, 1: 0 })];
  results.master = compare((await render()).pcm, baseline.pcm, gain, "master insert");
  project.master.inserts = [];
  channel.effectChain = [config({ 1: 1 })];
  const saved = join(root, "saved");
  await project.save(saved);
  const restored = await Project.load(saved);
  assert.deepEqual(restored.snapshot().channels[0].effectChain[0].state, channel.effectChain[0].state);
  await registerVst3Plugin(restored, source, host);
  results.saved = compare((await render(restored)).pcm, baseline.pcm, 1, "saved configuration");
  // Exercise session parameter events and non-block-aligned boundaries in the same graph.
  const session = await project.compile({ audioBackend: "simulated", latencyMode: "direct", renderAheadBlocks: 16 });
  await session.setParameter(channel.id, "insert.0.parameter.0", 0.75, 73);
  await session.setParameter(channel.id, "insert.0.parameter.1", 0, 73);
  const automatedPath = join(root, "automated.wav");
  await session.renderWav({ path: automatedPath, bitDepth: "float32", dither: "none" });
  const automated = await pcm(automatedPath);
  const latency = Number(baseline.report.graphLatencyFrames);
  for (let frame = 0; frame < automated.length / 2; frame++) {
    // The master limiter's oversampling FIR spreads the gain transition around its group delay.
    if (Math.abs(frame - (73 + latency)) <= 32) continue;
    const ratio = frame < 73 + latency ? 1 : gain;
    assert.ok(Math.abs(automated[frame * 2] - baseline.pcm[frame * 2] * ratio) < 0.000002, `automation frame ${frame}`);
  }
  results.automation = { frame: 73, graphLatencyFrames: latency, limiterTransitionMarginFrames: 32 };
  await session.play();
  await new Promise((resolve) => setTimeout(resolve, 400));
  const playing = await session.diagnostics();
  assert.equal(playing.state, "playing");
  assert.ok(playing.blocks > 0);
  assert.equal(playing.nanBlocks, 0);
  assert.ok(playing.events.some((event) => event.code === "ModeFallback"));
  await session.seek({ bar: 1 });
  await new Promise((resolve) => setTimeout(resolve, 150));
  await session.pause();
  await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal((await session.diagnostics()).state, "paused");
  await session.play();
  await new Promise((resolve) => setTimeout(resolve, 100));
  await session.play({ bar: 1 }, { startFrame: 0, endFrame: 12000 });
  await new Promise((resolve) => setTimeout(resolve, 2100));
  const looping = await session.diagnostics();
  assert.equal(looping.state, "playing");
  assert.equal(looping.nanBlocks, 0);
  assert.ok(looping.blocks > playing.blocks + 700, "looped graph must keep rendering");
  await session.stop();
  results.simulated = { playing, looping, final: await session.diagnostics(), faults: session.pluginDiagnostics() };
  assert.equal(results.simulated.faults[0].faults, 0);
  const report = resolve(process.env.OXITONE_VST3_PROJECT_REPORT ?? "target/vst3-project.json");
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
        results,
      },
      null,
      2,
    )}\n`,
  );
  // Keep the measured report even when the simulated audio deadline regression fails.
  for (const [phase, diagnostics] of Object.entries({ playing, looping, final: results.simulated.final })) {
    for (const key of ["xruns", "deadlineMisses", "nanBlocks", "queueDrops"])
      assert.equal(diagnostics[key], 0, `${phase}: ${key}; see ${report}`);
  }
  console.log(
    "VST3 project smoke passed: PCM bypass/gain/mix/serial/master/state/automation and simulated playback/seek/loop/pause/stop with no xruns or deadline misses",
  );
} finally {
  await project.session?.dispose();
  await rm(root, { recursive: true, force: true });
}
