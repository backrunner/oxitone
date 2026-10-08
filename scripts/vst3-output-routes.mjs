// SDK -> native facade -> isolated VST3 -> mixer route conformance. Never opens an audio device.
import assert from "node:assert/strict";
import { join } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { inspectVst3Plugin, registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";
import { captureOutputRoutes } from "./vst3-output-preview.mjs";

export async function checkOutputRoutes(bundlePath, host, root) {
  const source = { bundlePath, classId: "6E33225254224A00AA69301AF3187980", allowPlugins: "any" };
  const info = await inspectVst3Plugin(source, host);
  assert.equal(info.noteInput, true);
  assert.deepEqual(info.audioBuses, {
    inputs: [],
    outputs: [
      { channels: 2, active: true },
      { channels: 1, active: false },
      { channels: 2, active: true },
    ],
  });
  const project = new Project({ seed: 972 });
  const registered = await registerVst3Plugin(project, source, host);
  assert.equal(registered.kind, "instrument");
  const buses = Array.from({ length: 3 }, () => project.addMixerChannel({ masterSendRatio: 0 }));
  const channel = project.addChannel({
    mixerChannelId: buses[0].id,
    instrument: vst3Config(registered),
    outputRoutes: { 1: buses[1].id, 2: buses[2].id },
  });
  const notes = new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0.125, duration: 0.5, velocity: 0.5 }] });
  project.addTrack().use(channel).add(notes).at({ bar: 1 });
  let counter = 0;
  const render = async () => {
    const path = join(root, `outputs-${counter++}.wav`);
    await project.renderWav({ path, bitDepth: "float32", dither: "none" });
    return pcm(path);
  };
  const select = (index) =>
    buses.forEach((bus, i) => {
      bus.masterSendRatio = i === index ? 1 : 0;
    });
  select(0);
  const baseline = await render();
  const checks = [];
  for (const index of [1, 2]) {
    select(index);
    checks.push(compare(await render(), baseline, index + 1, `instrument output ${index}`));
  }
  channel.level = 0.5;
  checks.push(compare(await render(), baseline, 1.5, "auxiliary output channel fader"));
  channel.mute = true;
  assert.ok(
    (await render()).every((v) => v === 0),
    "mute gates auxiliary outputs",
  );
  channel.mute = false;
  channel.level = 1;
  channel.outputRoutes = { 1: buses[2].id };
  checks.push(compare(await render(), baseline, 2, "reroute bus 1 and deactivate bus 2"));
  channel.outputRoutes = {};
  assert.ok(
    (await render()).every((v) => v === 0),
    "removed routes contribute no audio",
  );
  channel.outputRoutes = { 1: buses[1].id, 2: buses[2].id };
  buses.forEach((bus) => {
    bus.masterSendRatio = 1;
  });
  const summed = await render();
  checks.push(compare(summed, baseline, 6, "sum all outputs once"));
  const saved = join(root, "saved-outputs");
  await project.save(saved);
  const restored = await Project.load(saved);
  await registerVst3Plugin(restored, source, host);
  const path = join(root, "restored-outputs.wav");
  await restored.renderWav({ path, bitDepth: "float32", dither: "none" });
  const reopened = await pcm(path);
  assert.equal(reopened.length, summed.length);
  assert.ok(
    reopened.every((sample, index) => Object.is(sample, summed[index])),
    "saved output routing reproduces PCM exactly",
  );
  const stems = await project.renderWav({
    path: join(root, "output-stems"),
    stems: "mixer-channels",
    bitDepth: "float32",
    dither: "none",
  });
  const primary = await pcm(stems.files.find((file) => file.stem === buses[0].id).path);
  for (const index of [1, 2]) {
    checks.push(
      compare(
        await pcm(stems.files.find((file) => file.stem === buses[index].id).path),
        primary,
        index + 1,
        `mixer stem ${index}`,
      ),
    );
  }
  const tracks = await project.renderWav({
    path: join(root, "output-track"),
    stems: "tracks",
    bitDepth: "float32",
    dither: "none",
  });
  checks.push(
    compare(await pcm(tracks.files.find((file) => file.stem).path), summed, 1, "track stem includes auxiliary outputs"),
  );
  const session = await project.compile({ audioBackend: "simulated", latencyMode: "direct", renderAheadBlocks: 16 });
  let diagnostics;
  try {
    await session.play({ bar: 1 }, { startFrame: 0, endFrame: 18000 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    channel.outputRoutes = { 2: buses[1].id };
    await session.update();
    await session.seek({ bar: 1 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    await session.stop();
    diagnostics = await session.diagnostics();
    assert.ok(diagnostics.blocks > 100, "multi-output graph renders through the simulated sink");
    // Worker timing stays in the report; an over-budget block is not a ring underrun.
    for (const key of ["xruns", "nanBlocks", "queueDrops"]) assert.equal(diagnostics[key], 0, key);
    assert.ok(session.pluginDiagnostics().every((plugin) => plugin.faults === 0));
  } finally {
    await session.dispose();
  }
  channel.outputRoutes = { 1: buses[1].id, 2: buses[2].id };
  const previewCapture = await captureOutputRoutes(project, root);
  return {
    checks,
    mute: true,
    removedRoutesSilent: true,
    restoredSamples: summed.length,
    simulated: diagnostics,
    previewCapture,
  };
}
