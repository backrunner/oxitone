// Real asymmetric 3-input/3-output VST3 routed by stable Mixer insert identity. No device output.
import assert from "node:assert/strict";
import { join } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";
import { checkInsertDocument } from "./vst3-insert-document.mjs";

export async function checkInsertRoutes(bundlePath, host, root) {
  const source = { bundlePath, classId: "6E33225254224A00AA69301AF318797F", allowPlugins: "any" };
  const project = new Project({ seed: 348 });
  const plugin = await registerVst3Plugin(project, source, host);
  assert.equal(plugin.kind, "effect");
  const buses = Array.from({ length: 5 }, () => project.addMixerChannel({ masterSendRatio: 0 }));
  const [owner, key1, key2, aux1, aux2] = buses;
  for (const [i, bus] of [owner, key1, key2].entries()) {
    const channel = project.addChannel({ mixerChannelId: bus.id });
    project
      .addTrack()
      .use(channel)
      .add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60 + i * 7, start: 0.125, duration: 0.5, velocity: 0.4 }] }))
      .at({ bar: 1 });
  }
  let counter = 0;
  const render = async () => {
    const path = join(root, `insert-routes-${counter++}.wav`);
    await project.renderWav({ path, bitDepth: "float32", dither: "none" });
    return pcm(path);
  };
  const select = (chosen) =>
    buses.forEach((bus) => {
      bus.masterSendRatio = chosen.includes(bus) ? 1 : 0;
    });
  const baselines = [];
  for (const bus of [owner, key1, key2]) {
    select([bus]);
    baselines.push(await render());
  }
  select([key1]);
  const referenceStems = await project.renderWav({
    path: join(root, "insert-route-reference-stems"),
    stems: "mixer-channels",
    bitDepth: "float32",
    dither: "none",
  });
  const key1Stem = await pcm(referenceStems.files.find((file) => file.stem === key1.id).path);
  const instance = owner.addEffect(vst3Config(plugin));
  const routing = { inputs: { 1: key1.id, 2: key2.id }, outputs: { 1: aux1.id, 2: aux2.id } };
  owner.routeInsert(instance, routing);
  const checks = [];
  select([owner]);
  const expectedMain = baselines[0].map((value, i) => value + baselines[2][i] * Math.SQRT1_2 * 0.125);
  checks.push(compare(await render(), expectedMain, 1, "main plus independent auxiliary input 2"));
  select([aux1]);
  checks.push(compare(await render(), baselines[1], 0.25, "mono input 1 to mono output 1"));
  select([aux2]);
  checks.push(compare(await render(), baselines[2], -0.125, "stereo input 2 to stereo output 2"));
  instance.host.param("mix").set(0.5);
  checks.push(compare(await render(), baselines[2], -0.0625, "auxiliary wet mix"));
  instance.host.param("bypass").set(true);
  assert.ok(
    (await render()).every((v) => v === 0),
    "bypass silences auxiliary outputs",
  );
  instance.host.param("bypass").set(false);
  instance.host.param("mix").set(1);
  owner.level = 0.5;
  checks.push(compare(await render(), baselines[2], -0.0625, "owner fader gates auxiliary outputs"));
  owner.mute = true;
  assert.ok((await render()).every((v) => v === 0));
  owner.mute = false;
  owner.level = 1;
  owner.routeInsert(instance, { inputs: { 1: key1.id }, outputs: { 2: aux2.id } });
  assert.ok(
    (await render()).every((v) => v === 0),
    "unrouted default-active input 2 is deactivated",
  );
  owner.routeInsert(instance, routing);
  select([owner, aux1, aux2]);
  const summed = await render();
  const expectedSum = expectedMain.map((v, i) => v + baselines[1][i] * 0.25 - baselines[2][i] * 0.125);
  checks.push(compare(summed, expectedSum, 1, "sum main and both auxiliaries once"));
  const saved = join(root, "saved-insert-routes");
  await project.save(saved);
  const restored = await Project.load(saved);
  await registerVst3Plugin(restored, source, host);
  const path = join(root, "restored-insert-routes.wav");
  await restored.renderWav({ path, bitDepth: "float32", dither: "none" });
  assert.deepEqual(await pcm(path), summed);
  // Direct auxiliary -> Master contributes to the owner's stem, independent of masterSendRatio.
  owner.routeInsert(instance, { inputs: routing.inputs, outputs: { 1: "mix_master" } });
  select([]);
  const direct = await render();
  checks.push(compare(direct, baselines[1], 0.5 * Math.SQRT1_2, "auxiliary direct to Master"));
  const stems = await project.renderWav({
    path: join(root, "insert-route-stems"),
    stems: "mixer-channels",
    bitDepth: "float32",
    dither: "none",
  });
  checks.push(
    compare(
      await pcm(stems.files.find((file) => file.stem === owner.id).path),
      key1Stem,
      0.5 * Math.SQRT1_2,
      "owner stem includes direct auxiliary at the pre-Master tap",
    ),
  );
  owner.routeInsert(instance, routing);
  select([owner, aux1, aux2]);
  const session = await project.compile({ audioBackend: "simulated", latencyMode: "direct", renderAheadBlocks: 16 });
  let diagnostics;
  try {
    await session.play({ bar: 1 }, { startFrame: 0, endFrame: 18000 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    owner.routeInsert(instance, { inputs: routing.inputs, outputs: { 1: key1.id } });
    await assert.rejects(session.update(), (error) => error.code === "InvalidProject");
    owner.routeInsert(instance, { inputs: routing.inputs, outputs: { 1: aux2.id } });
    await session.update();
    await session.seek({ bar: 1 });
    await new Promise((resolve) => setTimeout(resolve, 500));
    await session.stop();
    diagnostics = await session.diagnostics();
    assert.ok(diagnostics.blocks > 100);
    // Worker timing stays in the report; an over-budget block is not a ring underrun.
    for (const key of ["xruns", "nanBlocks", "queueDrops"]) assert.equal(diagnostics[key], 0, key);
    assert.ok(session.pluginDiagnostics().every((entry) => entry.faults === 0));
  } finally {
    await session.dispose();
  }
  owner.routeInsert(instance, { inputs: { 3: key1.id } });
  await assert.rejects(
    project.compile({ audioBackend: "simulated" }),
    (error) => error.code === "PluginCapabilityUnsupported",
  );
  return {
    sourceTransactions: await checkInsertDocument(project.registeredVst3Plugins[0], root),
    checks,
    restoredSamples: summed.length,
    bypass: true,
    mute: true,
    inactiveInputSilent: true,
    rejectedCycle: true,
    rejectedMissingBus: true,
    simulated: diagnostics,
  };
}
