// Actual pointer-backed plugin output -> SysEx thru -> audio receiver, entirely offline/simulated.
import assert from "node:assert/strict";
import { access } from "node:fs/promises";
import { join } from "node:path";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { registerVst3Plugin, renderVst3Wav, vst3Config } from "../packages/vst3/dist/index.js";
import { compare, pcm } from "./vst3-project-audio.mjs";

export async function checkSysEx(source, host, root) {
  const project = new Project({ seed: 147 });
  const emitter = await registerVst3Plugin(project, source("798C"), host);
  const thru = await registerVst3Plugin(project, source("798A"), host);
  const synth = await registerVst3Plugin(project, source("7990"), host);
  const destination = project.addChannel({ instrument: vst3Config(synth) });
  const origin = project.addChannel({ instrument: vst3Config(emitter) });
  const relay = project.addChannel({ instrument: vst3Config(thru) });
  origin.routeMidi(origin.instrumentInstance, [relay]);
  relay.routeMidi(relay.instrumentInstance, [destination]);
  const notes = new Pattern({
    lengthBeats: 1,
    notes: [
      { pitch: 60, start: 13 / 24000, duration: 96 / 24000, velocity: 64 / 127 },
      { pitch: 64, start: 0.25, duration: 0.25, velocity: 87 / 127 },
    ],
  });
  project.addTrack().use(origin).add(notes).at({ bar: 1 });
  const reference = new Project({ seed: 147 });
  reference.registerVst3(project.registeredVst3Plugins.find((p) => p.source.classId.endsWith("7990")));
  reference
    .addTrack()
    .use(reference.addChannel({ instrument: vst3Config(synth) }))
    .add(notes)
    .at({ bar: 1 });
  let serial = 0;
  const render = async (p) => {
    const path = join(root, `sysex-project-${serial++}.wav`);
    await p.renderWav({ path, end: { seconds: 0.5 }, tailSeconds: 0, bitDepth: "float32", dither: "none" });
    return pcm(path);
  };
  const baseline = await render(reference);
  const checks = [compare(await render(project), baseline, 1, "SysEx chain preserves offsets and copied bytes")];
  origin.mute = true;
  checks.push(compare(await render(project), baseline, 1, "SysEx is not cut by mute"));
  const second = project.addChannel({ instrument: vst3Config(synth) });
  relay.routeMidi(relay.instrumentInstance, [destination, second]);
  checks.push(compare(await render(project), baseline, 2, "SysEx fan-out reaches both instruments"));
  relay.routeMidi(relay.instrumentInstance, [destination]);
  const saved = join(root, "sysex-saved");
  await project.save(saved);
  const reopened = await Project.load(saved);
  for (const registration of project.registeredVst3Plugins) reopened.registerVst3(registration);
  checks.push(compare(await render(reopened), baseline, 1, "SysEx graph survives save/reopen"));

  const wireNotes = [
    { type: "noteOn", frame: 13, channel: 0, pitch: 60, velocity: 64 / 127 },
    { type: "noteOff", frame: 109, channel: 0, pitch: 60, velocity: 0 },
  ];
  const renderDirect = async (events, name) => {
    const path = join(root, `${name}.wav`);
    await renderVst3Wav(source("7990"), { path, frames: 256, events }, host);
    return pcm(path);
  };
  checks.push(
    compare(
      await renderDirect(
        [
          { type: "sysEx", frame: 13, data: [0xf0, 0x7d, 1, 64, 0xf7] },
          { type: "sysEx", frame: 109, data: [0xf0, 0x7d, 1, 0, 0xf7] },
        ],
        "sysex-direct",
      ),
      await renderDirect(wireNotes, "sysex-notes"),
      1,
      "standalone WAV accepts SysEx input",
    ),
  );
  const tooMuch = join(root, "sysex-overflow.wav");
  await assert.rejects(
    renderVst3Wav(
      source("7990"),
      {
        path: tooMuch,
        frames: 128,
        events: Array.from({ length: 5 }, () => ({
          type: "sysEx",
          frame: 0,
          data: [0xf0, ...Array(4094).fill(1), 0xf7],
        })),
      },
      host,
    ),
    { code: "BudgetExceeded" },
  );
  await assert.rejects(access(tooMuch), { code: "ENOENT" });

  const session = await project.compile({ audioBackend: "simulated", renderAheadBlocks: 16 });
  let diagnostics;
  try {
    await session.play({ bar: 1 }, { startFrame: 0, endFrame: 6000 });
    await new Promise((resolve) => setTimeout(resolve, 300));
    await session.seek({ bar: 1 });
    await new Promise((resolve) => setTimeout(resolve, 300));
    await session.stop();
    const deadline = performance.now() + 1000;
    do {
      diagnostics = await session.diagnostics();
      if (diagnostics.state === "stopped") break;
      await new Promise((resolve) => setTimeout(resolve, 5));
    } while (performance.now() < deadline);
    assert.equal(diagnostics.state, "stopped");
    assert.ok(diagnostics.blocks > 100);
    for (const key of ["xruns", "deadlineMisses", "nanBlocks", "queueDrops"]) assert.equal(diagnostics[key], 0, key);
    assert.ok(session.pluginDiagnostics().every((p) => p.faults === 0));
  } finally {
    await session.dispose();
  }
  return { checks, diagnostics, standaloneInput: true, outputOwnership: true, overflowNoFile: true };
}
