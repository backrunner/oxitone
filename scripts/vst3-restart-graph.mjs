// Real configured instances, failed-candidate retention, PDC and persisted replay; no device.
import assert from "node:assert/strict";
import { join } from "node:path";
import { Project, Pattern, createAutomationNamespace } from "../packages/core/dist/index.js";
import { inspectVst3Plugin, registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";
import { verifyLiveDaw } from "./vst3-daw-instance.mjs";

export async function verifyRestartGraph(source, host, root) {
  const project = new Project({ seed: 841 });
  const registration = await registerVst3Plugin(project, source, host);
  const channels = [project.addChannel(), project.addChannel()];
  for (const channel of channels) {
    channel.addEffect(vst3Config(registration));
    project
      .addTrack()
      .use(channel)
      .add(new Pattern({ lengthBeats: 2, notes: [{ pitch: 60, start: 0.25, duration: 0.5, velocity: 0.1 }] }))
      .at({ bar: 1 });
  }
  let renderIndex = 0;
  const render = async (owner) => {
    const path = join(root, `restart-${renderIndex++}.wav`);
    const report = await owner.renderWav({ path, bitDepth: "float32", dither: "none" });
    return { report, samples: await pcm(path) };
  };
  const baseline = await render(project);
  channels[1].mute = true;
  const firstPath = await render(project);
  channels[1].mute = false;
  const session = await project.compile({ allowPlugins: "any", audioBackend: "simulated", renderAheadBlocks: 16 });
  const first = channels[0].effectChain[0],
    other = channels[1].effectChain[0];
  const target = (id) => ({ graphGeneration: session.vst3Instances().graphGeneration, instanceId: id });
  const originalTarget = target(first.instanceId);
  let restored, diagnostics;
  try {
    const result = await session.controlVst3Instance(originalTarget, {
      kind: "setParameter",
      parameterId: 0,
      value: 1 / 3,
    });
    assert.equal(result.state.restart.latencyFrames, 64);
    const capture = (await session.controlVst3Instance(originalTarget, { kind: "capture" })).state.info;
    assert.equal(capture.outputChannels, 1);
    assert(capture.parameters.some((p) => p.id === 7 && p.canAutomate));
    // Invalid new configuration must leave the frozen helper and its recovery state reachable.
    channels[0].effectChain = [{ ...first, parameters: { 999: 0.5 }, state: capture.configuration }];
    await assert.rejects(session.update(), { code: "PluginConfigInvalid" });
    assert.equal(session.vst3Instances().graphGeneration, originalTarget.graphGeneration);
    assert.deepEqual((await session.controlVst3Instance(originalTarget, { kind: "capture" })).state.info, capture);
    const config = vst3Config(registration, { configuration: capture.configuration, parameters: { 7: 0.25 } });
    channels[0].effectChain = [{ ...config, instanceId: first.instanceId }];
    await session.update();
    await assert.rejects(session.controlVst3Instance(originalTarget, { kind: "poll" }), { code: "PluginTaskConflict" });
    const current = (await session.controlVst3Instance(target(first.instanceId), { kind: "capture" })).state;
    assert.equal(current.restart, undefined);
    assert.equal(current.info.configuration.parameters[7], 0.25);
    assert.equal(
      (await session.controlVst3Instance(target(other.instanceId), { kind: "capture" })).state.info.outputChannels,
      2,
    );
    const next = await render(session);
    assert.equal(next.report.graphLatencyFrames - baseline.report.graphLatencyFrames, 64);
    const shifted = baseline.samples.map((_, i) =>
      i < 128 ? 0 : baseline.samples[i - 128] - firstPath.samples[i - 128] * 0.5,
    );
    const pdc = compare(next.samples, shifted, 1, "dynamic latency aligns both channel paths");
    channels[0].effectInstances[0].param("7").automate(createAutomationNamespace().constant(0.25));
    await session.update();
    const automated = await render(session);
    compare(automated.samples, next.samples, 1, "new parameter automation uses the instance descriptor");
    // The same parameter is invalid on the unchanged instance of the same class.
    channels[1].effectChain = [{ ...other, parameters: { 7: 0.25 } }];
    const accepted = session.vst3Instances().graphGeneration;
    await assert.rejects(session.update(), { code: "PluginConfigInvalid" });
    assert.equal(session.vst3Instances().graphGeneration, accepted);
    channels[1].effectChain = [other];
    const saved = join(root, "restart-project");
    await project.save(saved);
    restored = await Project.load(saved);
    await registerVst3Plugin(restored, source, host);
    assert.deepEqual((await render(restored)).samples, next.samples);
    // Publish another fully prepared graph while the native worker is running.
    await session.play({ frame: 0 }, { startFrame: 0, endFrame: 48000 });
    await new Promise((resolve) => setTimeout(resolve, 200));
    await session.update();
    await new Promise((resolve) => setTimeout(resolve, 200));
    diagnostics = await session.diagnostics();
    assert.equal(diagnostics.state, "playing");
    assert.equal(diagnostics.nanBlocks, 0);
    assert(session.pluginDiagnostics().every((plugin) => plugin.faults === 0));
    await session.stop();
    const metadata = await inspectVst3Plugin(source, host);
    const daw = await verifyLiveDaw(source, metadata, host, { parameterId: 0, initial: 0, changed: 1 / 3 });
    return {
      pdc,
      daw,
      diagnostics,
      checks: [
        "configuredInstanceIsolation",
        "failedCompilePreservesFrozenState",
        "newBusLayoutAndParameter",
        "newParameterAutomation",
        "pdcRecompiled",
        "savedPcmExact",
        "playingGraphSwap",
      ],
    };
  } finally {
    await session.dispose();
    await restored?.session?.dispose();
  }
}
