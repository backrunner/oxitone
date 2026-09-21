import assert from "node:assert/strict";
import { Project, Pattern } from "../packages/core/dist/index.js";
import { registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";

const pause = () => new Promise((resolve) => setTimeout(resolve, 5));
async function until(read, accept) {
  const end = performance.now() + 5000;
  while (performance.now() < end) {
    const value = await read();
    if (accept(value)) return value;
    await pause();
  }
  throw new Error("VST3 gesture SDK conformance timed out");
}
export async function verifyEditSdk(source, hostPath) {
  const project = new Project({ seed: 272 });
  try {
    const registration = await registerVst3Plugin(project, source, { hostPath });
    const channel = project.addChannel();
    channel.effectChain = [vst3Config(registration)];
    project
      .addTrack()
      .use(channel)
      .add(new Pattern({ lengthBeats: 4, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.1 }] }))
      .at({ bar: 1 });
    const session = await project.compile({ audioBackend: "simulated", renderAheadBlocks: 16 });
    const inventory = await until(
      () => session.vst3Instances(),
      (value) => value.state === "active",
    );
    const target = { graphGeneration: inventory.graphGeneration, instanceId: channel.effectChain[0].instanceId };
    const control = (command) => session.controlVst3Instance(target, command);
    const original = project.snapshot();
    const captureId = (await control({ kind: "startEdits" })).state.edits.captureId;
    await assert.rejects(control({ kind: "startEdits" }), { code: "PluginTaskConflict" });
    await control({ kind: "setParameter", parameterId: 99, value: 0.125 });
    const read = (fromSequence) =>
      control({ kind: "readEdits", captureId, fromSequence }).then((result) => result.state.edits);
    assert.equal((await read(0)).pendingEvents, 4);
    await session.play({ bar: 1 });
    const page = await until(
      () => read(0),
      (value) => value.events.length === 4,
    );
    assert.deepEqual(
      page.events.map((event) => event.kind),
      ["begin", "value", "value", "end"],
    );
    assert.deepEqual(
      page.events.filter((event) => event.kind === "value").map((event) => event.value),
      [0.25, 0.75],
    );
    assert(page.events.every((event) => event.position.transport.playing));
    assert.deepEqual((await read(0)).events, page.events);
    const cursor = page.firstSequence + page.events.length;
    await control({ kind: "stopEdits", captureId, fromSequence: cursor });
    const stopped = await until(
      () => read(cursor),
      (value) => value.status === "stopped",
    );
    assert(stopped.endPosition.audioSequence >= page.events[3].position.audioSequence);
    assert.deepEqual(project.snapshot(), original, "gesture collection must not edit authoring state");
    await session.pause();
    await assert.rejects(read(0), { code: "PluginTaskConflict" });
    await control({ kind: "discardEdits", captureId });
    await assert.rejects(read(cursor), { code: "PluginTaskConflict" });
    await control({ kind: "startEdits" });
    await session.update();
    await until(
      () => session.vst3Instances(),
      (value) => value.state === "active" && value.graphGeneration !== target.graphGeneration,
    );
    await assert.rejects(control({ kind: "readEdits", captureId, fromSequence: cursor }), {
      code: "PluginTaskConflict",
    });
    const diagnostics = await session.diagnostics();
    const faults = session.pluginDiagnostics();
    for (const key of ["xruns", "deadlineMisses", "nanBlocks", "queueDrops"]) assert.equal(diagnostics[key], 0, key);
    assert(faults.every((plugin) => plugin.faults === 0));
    return {
      checks: [
        "nativeFacadeEdits",
        "singleActiveCapture",
        "preplayPending",
        "simulatedClock",
        "retryPage",
        "stopBoundary",
        "sourceUnchanged",
        "acknowledgedCursorRejected",
        "discard",
        "retiredGraphRejected",
      ],
      page,
      stopped,
      diagnostics,
      faults,
    };
  } finally {
    await project.session?.dispose();
  }
}
