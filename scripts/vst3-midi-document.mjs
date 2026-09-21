// Accepted source transactions preserve stable MIDI endpoint identities through Undo/Save.
import assert from "node:assert/strict";
import { mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { ProjectDocument } from "../packages/cli/dist/source/index.js";

export async function checkMidiDocument(registrations, root) {
  const directory = join(root, "midi-document");
  await mkdir(join(directory, "node_modules/@oxitone"), { recursive: true });
  await symlink(resolve("packages/core"), join(directory, "node_modules/@oxitone/core"), "dir");
  await writeFile(join(directory, "registrations.json"), JSON.stringify(registrations));
  const config = (suffix) => {
    const registration = registrations.find((r) => r.source.classId.toLowerCase().endsWith(suffix));
    return JSON.stringify({
      pluginId: `vst3.${registration.source.classId.toLowerCase()}`,
      pluginVersion: `0.0.0+${registration.metadata.sha256}`,
      parameters: {},
    });
  };
  const entry = join(directory, "song.ts");
  const original = `import { Project } from '@oxitone/core';
import registrations from './registrations.json';
const project = new Project();
for (const registration of registrations) project.registerVst3(registration);
const receiver = project.addChannel({ name: 'Receiver', instrument: ${config("7980")} });
const owner = project.addChannel({ name: 'Owner' });
const instance = owner.addEffect(${config("7987")});
owner.routeMidi(instance, [receiver]);
export default project;
`;
  await writeFile(entry, original);
  let document;
  try {
    document = await ProjectDocument.open({ entry });
    assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
    const owner = () => document.frame.snapshot.channels.find((c) => c.name === "Owner");
    const instance = owner().effectChain[0].instanceId;
    const routes = structuredClone(owner().midiRoutes);
    const site = document.view.configurationSites.find(
      (site) => site.scope === "reference" && site.usages.length === 1 && site.usages[0].handle === instance,
    );
    assert.ok(site);
    await document.editConfiguration(
      document.view.revision,
      site.handle,
      { kind: "host", values: { mix: 0.5 } },
      instance,
    );
    assert.equal(owner().effectChain[0].mix, 0.5);
    assert.deepEqual(owner().midiRoutes, routes);
    assert.equal(await readFile(entry, "utf8"), original);
    await document.undo(document.view.revision);
    assert.equal(owner().effectChain[0].mix, undefined);
    assert.deepEqual(owner().midiRoutes, routes);
    await document.redo(document.view.revision);
    await document.save(document.view.revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
    assert.equal(owner().effectChain[0].mix, 0.5);
    assert.equal(owner().effectChain[0].instanceId, instance);
    assert.deepEqual(owner().midiRoutes, routes);
    return { configurationEdit: true, undoRedo: true, saveReopen: true, stableRoutes: true };
  } finally {
    document?.close();
  }
}
