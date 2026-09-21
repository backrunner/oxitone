// CI conformance of the actual SDK/native graph with the source-built dense VST3 processor.
import assert from "node:assert/strict";
import { Project } from "../packages/core/dist/index.js";
import { configureVst3Plugin, registerVst3Plugin, vst3Config } from "../packages/vst3/dist/index.js";

export async function verifyDenseInstance(source, metadata, host) {
  const project = new Project();
  const registration = await registerVst3Plugin(project, source, host);
  project.master.inserts = [
    vst3Config(registration, { configuration: metadata.configuration }),
    vst3Config(registration, { configuration: metadata.configuration }),
  ];
  const [first, second] = project.master.inserts;
  const session = await project.compile({ allowPlugins: "any", audioBackend: "simulated" });
  try {
    const inventory = session.vst3Instances();
    assert.equal(inventory.state, "active");
    assert.deepEqual(
      inventory.instances.map((i) => i.instanceId),
      [first.instanceId, second.instanceId].toSorted(),
    );
    const target = (id) => ({ graphGeneration: inventory.graphGeneration, instanceId: id });
    await session.controlVst3Instance(target(first.instanceId), {
      kind: "setParameter",
      parameterId: 4095,
      value: 0.375,
    });
    const captured = await session.controlVst3Instance(target(first.instanceId), { kind: "capture" });
    const other = await session.controlVst3Instance(target(second.instanceId), { kind: "capture" });
    assert.equal(captured.state.nextSequence, 0);
    assert.equal(captured.state.info.configuration.parameters["4095"], 0.375);
    assert.equal(other.state.info.configuration.parameters["4095"], 0.5);
    const restored = await configureVst3Plugin(
      source,
      {
        configuration: { ...captured.state.info.configuration, parameters: {} },
      },
      host,
    );
    assert.equal(restored.parameters.find((p) => p.id === 4095).value, 0.375);
    const stale = target(first.instanceId);
    const pending = session.controlVst3Instance(stale, { kind: "capture" }).then(
      () => null,
      (error) => error,
    );
    await session.update();
    assert.equal((await pending)?.code, "PluginTaskConflict");
    assert.notEqual(session.vst3Instances().graphGeneration, stale.graphGeneration);
    await assert.rejects(session.controlVst3Instance(stale, { kind: "poll" }), { code: "PluginTaskConflict" });
  } finally {
    await session.dispose();
  }
}
