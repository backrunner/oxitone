// MIDI-only Channel configurations remain ordinary source transactions with stable routes.
import assert from "node:assert/strict";
import { mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { ProjectDocument } from "../packages/cli/dist/source/index.js";

export async function checkMidiOnlyDocument(registrations, root) {
  const directory = join(root, "midi-only-document");
  await mkdir(join(directory, "node_modules/@oxitone"), { recursive: true });
  await symlink(resolve("packages/core"), join(directory, "node_modules/@oxitone/core"), "dir");
  await writeFile(join(directory, "registrations.json"), JSON.stringify(registrations));
  const config = (suffix) => {
    const registration = registrations.find((r) => r.source.classId.toLowerCase().endsWith(suffix));
    return JSON.stringify({
      pluginId: `vst3.${registration.source.classId.toLowerCase()}`,
      pluginVersion: `0.0.0+${registration.metadata.sha256}`,
      parameters: { 0: 1 },
    });
  };
  const entry = join(directory, "song.ts");
  const original = `import { Project } from '@oxitone/core';
import registrations from './registrations.json';
const project = new Project();
for (const registration of registrations) project.registerVst3(registration);
const receiver = project.addChannel({ name: 'Receiver', instrument: ${config("7980")} });
const owner = project.addChannel({ name: 'MidiOnly', instrument: ${config("798a")} });
owner.routeMidi(owner.instrumentInstance, [receiver]);
export default project;
`;
  await writeFile(entry, original);
  let document;
  try {
    document = await ProjectDocument.open({ entry });
    assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
    const owner = () => document.frame.snapshot.channels.find((c) => c.name === "MidiOnly");
    const instance = owner().instrument.instanceId;
    const routes = structuredClone(owner().midiRoutes);
    const entryInfo = document.view.plugins.find((p) => p.pluginId === owner().instrument.pluginId);
    assert.equal(entryInfo.kind, "instrument");
    await document.verifyPlugin(document.view.revision, entryInfo.handle);
    const verified = document.view.plugins.find((p) => p.handle === entryInfo.handle);
    assert.equal(verified.validation, "verified", verified.diagnostic ?? "MIDI-only verification failed");
    assert.equal(verified.vst3.outputChannels, 0);
    const site = document.view.configurationSites.find(
      (s) => s.scope === "reference" && s.usages.length === 1 && s.usages[0].handle === instance,
    );
    assert.ok(site);
    await document.editConfiguration(
      document.view.revision,
      site.handle,
      { kind: "parameters", values: { 0: 0.25 } },
      instance,
    );
    assert.equal(owner().instrument.parameters["0"], 0.25);
    assert.deepEqual(owner().midiRoutes, routes);
    assert.equal(await readFile(entry, "utf8"), original);
    await document.undo(document.view.revision);
    assert.equal(owner().instrument.parameters["0"], 1);
    await document.redo(document.view.revision);
    await document.save(document.view.revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
    assert.equal(owner().instrument.instanceId, instance);
    assert.equal(owner().instrument.parameters["0"], 0.25);
    assert.deepEqual(owner().midiRoutes, routes);
    return { configurationEdit: true, undoRedo: true, saveReopen: true, catalogKind: true };
  } finally {
    document?.close();
  }
}
