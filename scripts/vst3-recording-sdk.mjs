// A real recorded take is replayed through the SDK and compared sample by sample, then saved/reopened.
import assert from "node:assert/strict";
import { join } from "node:path";
import { readFile } from "node:fs/promises";
import { Project, Pattern, createAutomationNamespace } from "../packages/core/dist/index.js";
import { registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { pcm } from "./vst3-project-audio.mjs";

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(read, accept) {
  const end = performance.now() + 5000;
  while (performance.now() < end) {
    const value = await read();
    if (accept(value)) return value;
    await pause(5);
  }
  throw new Error("Recording conformance timed out");
}
export async function verifyRecordingSdk(source, hostPath, root) {
  const project = new Project({ seed: 273 });
  let restored;
  try {
    const plugin = await registerVst3Plugin(
      project,
      { ...source, classId: "6E33225254224A00AA69301AF3187983" },
      { hostPath },
    );
    const channel = project.addChannel();
    channel.effectChain = [vst3Config(plugin, { parameters: { 0: 0.2 } })];
    project
      .addTrack()
      .use(channel)
      .add(new Pattern({ lengthBeats: 4, notes: [{ pitch: 60, start: 0, duration: 3, velocity: 0.2 }] }))
      .at({ bar: 1 });
    const parameter = { entityId: channel.effectChain[0].instanceId, parameterId: "0", scope: "plugin" };
    const base = createAutomationNamespace().constant(0.2);
    const lane = project.addAutomationLane(parameter, base);
    const session = await project.compile({ audioBackend: "simulated", renderAheadBlocks: 16 });
    const inventory = await until(
      () => session.vst3Instances(),
      (value) => value.state === "active",
    );
    const target = { graphGeneration: inventory.graphGeneration, instanceId: parameter.entityId };
    const original = project.snapshot();
    const baseline = await session.renderWav({ path: join(root, "before.wav"), bitDepth: "float32", dither: "none" });
    const recorder = await session.recordVst3Automation(target, { mode: "write", parameterIds: [0] });
    await session.play({ bar: 1 }, { startFrame: 0, endFrame: 12000 });
    await pause(80);
    await session.controlVst3Instance(target, { kind: "setParameter", parameterId: 99, value: 0.125 });
    await pause(120);
    await session.seek({ bar: 1 });
    await pause(80);
    const take = await recorder.stop();
    assert.equal(recorder.active, false);
    assert(take.spans.some((span) => span.value === 0.75));
    assert(take.spans.some((span, index) => index > 0 && span.start < take.spans[index - 1].start));
    assert.deepEqual(project.snapshot(), original);
    const diagnostics = await session.diagnostics();
    await session.pause();
    const sourceOverlay = take.source(0, base);
    project.removeAutomationLane(lane);
    const recordedLane = project.addAutomationLane(parameter, sourceOverlay);
    await session.update();
    await session.renderWav({ path: join(root, "recorded.wav"), bitDepth: "float32", dither: "none" });
    const before = await pcm(join(root, "before.wav")),
      after = await pcm(join(root, "recorded.wav"));
    assert.equal(after.length, before.length);
    let peakError = 0,
      audibleSamples = 0;
    const gainAt = (frame) =>
      take.spans.reduce(
        (gain, span) =>
          frame >= Math.round(span.start * 24000) && frame < Math.round(span.end * 24000) ? span.value : gain,
        0.2,
      );
    const boundaries = [
      ...new Set([0, ...take.spans.flatMap((span) => [Math.round(span.start * 24000), Math.round(span.end * 24000)])]),
    ].sort((a, b) => a - b);
    const latency = Number(baseline.graphLatencyFrames);
    for (let sample = 0; sample < after.length; sample++) {
      const frame = Math.floor(sample / 2) - latency;
      // Master limiter FIR spreads each gain edge; the independent full-PCM reference below
      // covers these samples too. The native probe separately checks exact DSP sample offsets.
      if (boundaries.some((edge) => Math.abs(frame - edge) <= 40)) continue;
      const gain = gainAt(frame);
      peakError = Math.max(peakError, Math.abs(after[sample] - (before[sample] * gain) / 0.2));
      if (gain !== 0.2 && Math.abs(before[sample]) > 0.0001) audibleSamples++;
    }
    assert(audibleSamples > 1000);
    assert(peakError < 0.000002, `recorded source PCM mismatch: ${peakError}`);
    const saved = join(root, "recorded-project");
    await project.save(saved);
    restored = await Project.load(saved);
    await registerVst3Plugin(restored, { ...source, classId: "6E33225254224A00AA69301AF3187983" }, { hostPath });
    const reopened = await restored.compile({ audioBackend: "simulated" });
    await reopened.renderWav({ path: join(root, "reopened.wav"), bitDepth: "float32", dither: "none" });
    assert.deepEqual(await readFile(join(root, "reopened.wav")), await readFile(join(root, "recorded.wav")));
    // Independent reference: enumerate integer packet edges and select the last matching span,
    // then author a flat step curve. No recordedSource/replaceRange normalization is reused.
    project.removeAutomationLane(recordedLane);
    project.addAutomationLane(
      parameter,
      createAutomationNamespace().curve(
        boundaries.map((frame) => ({
          beat: frame / 24000,
          value: gainAt(frame),
          curve: { kind: "step" },
        })),
      ),
    );
    await session.update();
    await session.renderWav({ path: join(root, "reference.wav"), bitDepth: "float32", dither: "none" });
    assert.deepEqual(await readFile(join(root, "reference.wav")), await readFile(join(root, "recorded.wav")));
    const fresh = await until(
      () => session.vst3Instances(),
      (value) => value.state === "active",
    );
    const current = { ...target, graphGeneration: fresh.graphGeneration };
    const paused = await session.recordVst3Automation(current, { mode: "write", parameterIds: [0] });
    await assert.rejects(paused.stop({ timeoutMs: 50 }), { code: "PluginTaskConflict" });
    const cancelled = await session.recordVst3Automation(current, { mode: "write", parameterIds: [0] });
    await cancelled.cancel();
    await assert.rejects(cancelled.stop(), { code: "PluginTaskConflict" });
    const old = await session.recordVst3Automation(current, { mode: "touch", parameterIds: [0] });
    await session.update();
    await assert.rejects(old.stop(), { code: "PluginTaskConflict" });
    const faults = session.pluginDiagnostics();
    // Worker timing stays in the report; an over-budget block is not a ring underrun.
    for (const key of ["xruns", "nanBlocks", "queueDrops"]) assert.equal(diagnostics[key], 0, key);
    assert(faults.every((plugin) => plugin.faults === 0));
    return {
      checks: [
        "automaticCursorDrain",
        "writeOverExistingAutomation",
        "seekAndLoopSpans",
        "sourceUnchangedBeforeAccept",
        "stepOverlayMatchesPcm",
        "independentStepCurveExactPcm",
        "saveReopenExactPcm",
        "pausedStopFailsExplicitly",
        "cancel",
        "retiredGraphRejected",
      ],
      spans: take.spans,
      source: sourceOverlay.toSpec(),
      pcm: { samples: after.length, audibleSamples, peakError, graphLatencyFrames: latency, firMarginFrames: 40 },
      diagnostics,
      faults,
    };
  } finally {
    await restored?.session?.dispose();
    await project.session?.dispose();
  }
}
