// Real zero-audio-bus processors: MIDI chains, autonomous generation and live capture.
import assert from "node:assert/strict";
import { join } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import {
  registerVst3Plugin,
  inspectVst3Plugin,
  configureVst3Plugin,
  renderVst3Wav,
  vst3Config,
} from "../packages/vst3/dist/index.js";
import { pcm, compare } from "./vst3-project-audio.mjs";
import { checkMidiOnlyDocument } from "./vst3-midi-only-document.mjs";

export async function checkMidiOnly(source, host, root) {
  const project = new Project({ seed: 129 });
  const synth = await registerVst3Plugin(project, source("7980"), host);
  const thru = await registerVst3Plugin(project, source("798A"), host);
  const generator = await registerVst3Plugin(project, source("798B"), host);
  assert.equal(thru.kind, "instrument");
  assert.equal(generator.kind, "instrument");
  for (const suffix of ["798A", "798B"]) {
    const info = await inspectVst3Plugin(source(suffix), host);
    assert.deepEqual(info.audioBuses, { inputs: [], outputs: [] });
    assert.equal(info.outputChannels, 0);
    assert.equal(info.noteInput, suffix === "798A");
    const restored = await configureVst3Plugin(
      source(suffix),
      { configuration: info.configuration, parameters: { 0: 0.25 } },
      host,
    );
    assert.deepEqual(restored.audioBuses, info.audioBuses);
    assert.equal(restored.configuration.parameters["0"], 0.25);
    await assert.rejects(
      renderVst3Wav(source(suffix), { path: join(root, `unsupported-${suffix}.wav`), frames: 128 }, host),
      { code: "PluginCapabilityUnsupported" },
    );
  }
  const destination = project.addChannel({ instrument: vst3Config(synth) });
  const a = project.addChannel({ instrument: vst3Config(thru) });
  const b = project.addChannel({ instrument: vst3Config(thru) });
  const notes = new Pattern({
    lengthBeats: 1,
    notes: [
      { pitch: 60, start: 13 / 24000, duration: 96 / 24000, velocity: 64 / 127 },
      { pitch: 67, start: 0.25, duration: 0.25, velocity: 87 / 127 },
    ],
  });
  project.addTrack().use(a).add(notes).at({ bar: 1 });
  const reference = new Project({ seed: 129 });
  await registerVst3Plugin(reference, source("7980"), host);
  const target = reference.addChannel({ instrument: vst3Config(synth) });
  reference.addTrack().use(target).add(notes).at({ bar: 1 });
  let serial = 0;
  const render = async (p) => {
    const path = join(root, `midi-only-${serial++}.wav`);
    await p.renderWav({ path, end: { seconds: 0.5 }, tailSeconds: 0, bitDepth: "float32", dither: "none" });
    return pcm(path);
  };
  const baseline = await render(reference);
  assert.ok(
    (await render(project)).every((v) => v === 0),
    "unrouted MIDI-only instruments produce no audio",
  );
  a.routeMidi(a.instrumentInstance, [b]);
  b.routeMidi(b.instrumentInstance, [destination]);
  const checks = [compare(await render(project), baseline, 1, "two MIDI-only processors deliver to the synth")];
  a.mute = true;
  checks.push(compare(await render(project), baseline, 1, "MIDI-only channel mute preserves note delivery"));
  const saved = join(root, "midi-only-saved");
  await project.save(saved);
  const loaded = await Project.load(saved);
  for (const registration of project.registeredVst3Plugins) loaded.registerVst3(registration);
  checks.push(compare(await render(loaded), baseline, 1, "MIDI-only project save/load"));
  const autonomous = new Project({ seed: 129 });
  for (const registration of project.registeredVst3Plugins) autonomous.registerVst3(registration);
  const clock = autonomous.addChannel({ instrument: vst3Config(generator) });
  const sound = autonomous.addChannel({ instrument: vst3Config(synth) });
  clock.routeMidi(clock.instrumentInstance, [sound]);
  const generatedReference = new Project({ seed: 129 });
  await registerVst3Plugin(generatedReference, source("7980"), host);
  const voice = generatedReference.addChannel({ instrument: vst3Config(synth) });
  const generatedNotes = Array.from({ length: 94 }, (_, i) => ({
    pitch: 72,
    start: (256 * i + 13) / 24000,
    duration: 96 / 24000,
    velocity: 64 / 127,
  }));
  generatedReference
    .addTrack()
    .use(voice)
    .add(new Pattern({ lengthBeats: 2, notes: generatedNotes }))
    .at({ bar: 1 });
  checks.push(
    compare(
      await render(autonomous),
      await render(generatedReference),
      1,
      "autonomous MIDI clock matches authored notes",
    ),
  );
  const session = await autonomous.compile({ audioBackend: "simulated", renderAheadBlocks: 16 });
  let diagnostics;
  try {
    await session.play({ bar: 1 }, { startFrame: 0, endFrame: 6000 });
    await new Promise((resolve) => setTimeout(resolve, 300));
    const inventory = session.vst3Instances();
    const controlTarget = { graphGeneration: inventory.graphGeneration, instanceId: clock.instrumentInstance.id };
    const captured = await session.controlVst3Instance(controlTarget, { kind: "capture" });
    assert.deepEqual(captured.state.info.audioBuses, { inputs: [], outputs: [] });
    clock.instrument = {
      ...clock.instrument,
      state: captured.state.info.configuration,
      parameters: captured.state.info.configuration.parameters,
    };
    assert.deepEqual(clock.midiRoutes[clock.instrumentInstance.id], [sound.id]);
    await session.update();
    await session.seek({ bar: 1 });
    await new Promise((resolve) => setTimeout(resolve, 300));
    await session.stop();
    diagnostics = await session.diagnostics();
    assert.ok(diagnostics.blocks > 100);
    // Worker timing stays in the report; an over-budget block is not a ring underrun.
    for (const key of ["xruns", "nanBlocks", "queueDrops"]) assert.equal(diagnostics[key], 0, key);
    assert.ok(session.pluginDiagnostics().every((p) => p.faults === 0));
  } finally {
    await session.dispose();
  }
  autonomous.addTrack().use(clock).add(notes).at({ bar: 1 });
  await assert.rejects(autonomous.compile({ audioBackend: "simulated" }), { code: "PluginCapabilityUnsupported" });
  const sourceTransactions = await checkMidiOnlyDocument(project.registeredVst3Plugins, root);
  return {
    checks,
    sourceTransactions,
    diagnostics,
    configuration: true,
    liveCapture: true,
    rejectedMissingNoteInput: true,
    rejectedStandaloneWav: true,
  };
}
