// Source authoring -> configuration transaction -> Undo/Redo/Save, with owner-local bus routes intact.
import assert from "node:assert/strict";
import { mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { ProjectDocument } from "../packages/cli/dist/source/index.js";

export async function checkInsertDocument(registration, root) {
  const directory = join(root, "routed-document");
  await mkdir(join(directory, "node_modules/@oxitone"), { recursive: true });
  await symlink(resolve("packages/core"), join(directory, "node_modules/@oxitone/core"), "dir");
  await writeFile(join(directory, "registration.json"), JSON.stringify(registration));
  const entry = join(directory, "song.ts");
  const original = `import { Project } from '@oxitone/core';
import registration from './registration.json';
const project = new Project();
project.registerVst3(registration);
const input = project.addMixerChannel({ name: 'Input', masterSendRatio: 0 });
const output = project.addMixerChannel({ name: 'Output' });
const owner = project.addMixerChannel({ name: 'Owner' });
const instance = owner.addEffect({ pluginId: 'vst3.${registration.source.classId.toLowerCase()}', pluginVersion: '0.0.0+${registration.metadata.sha256}', parameters: {} });
owner.routeInsert(instance, { inputs: { 1: input.id, 2: input.id }, outputs: { 1: output.id, 2: 'mix_master' } });
export default project;
`;
  await writeFile(entry, original);
  let document;
  try {
    document = await ProjectDocument.open({ entry });
    assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
    const owner = () => document.frame.snapshot.mixerChannels.find((bus) => bus.name === "Owner");
    const instance = owner().inserts[0].instanceId;
    const routes = structuredClone(owner().insertRoutes);
    const site = document.view.configurationSites.find(
      (site) => site.scope === "reference" && site.usages.length === 1 && site.usages[0].handle === instance,
    );
    assert.ok(site, "routed insert needs a writable source configuration");
    await document.editConfiguration(
      document.view.revision,
      site.handle,
      { kind: "host", values: { mix: 0.5 } },
      instance,
    );
    assert.equal(owner().inserts[0].mix, 0.5);
    assert.deepEqual(owner().insertRoutes, routes);
    assert.equal(await readFile(entry, "utf8"), original);
    await document.undo(document.view.revision);
    assert.equal(owner().inserts[0].mix, undefined);
    assert.deepEqual(owner().insertRoutes, routes);
    await document.redo(document.view.revision);
    assert.equal(owner().inserts[0].mix, 0.5);
    await document.save(document.view.revision);
    document.close();
    document = await ProjectDocument.open({ entry });
    assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
    assert.equal(owner().inserts[0].instanceId, instance);
    assert.equal(owner().inserts[0].mix, 0.5);
    assert.deepEqual(owner().insertRoutes, routes);
    return { configurationEdit: true, undoRedo: true, saveReopen: true, stableRoutes: true };
  } finally {
    document?.close();
  }
}
