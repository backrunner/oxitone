// Source transaction + actual VST3 instrument. Optional native window uses simulated audio only.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, symlink, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { ProjectDocument } from "../packages/cli/dist/source/index.js";
import { Project } from "../packages/core/dist/index.js";
import { pcm } from "./vst3-project-audio.mjs";

const gainPath = process.env.OXITONE_VST3_FIXTURE;
const synthPath = process.env.OXITONE_VST3_INSTRUMENT;
if (!gainPath || !synthPath)
  throw new Error("Set OXITONE_VST3_FIXTURE (VestiGain) and OXITONE_VST3_INSTRUMENT (VestiMIDISynth)");
const helperPath = resolve(process.env.OXITONE_VST3_HOST_PATH ?? "target/release/oxitone-vst3-host");
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-daw-"));
let document;
try {
  await mkdir(join(root, "node_modules", "@oxitone"), { recursive: true });
  for (const name of ["core", "vst3"])
    await symlink(resolve(`packages/${name}`), join(root, "node_modules", "@oxitone", name), "dir");
  const entry = join(root, "song.ts");
  await writeFile(
    entry,
    `import { Project, Pattern } from '@oxitone/core';
import { registerVst3Plugin } from '@oxitone/vst3';
export default async function createProject() {
  const project = new Project({ name: 'VST3 assignment', seed: 117 });
  const channel = project.addChannel();
  project.addTrack().use(channel).add(new Pattern({ lengthBeats: 2, notes: [{ pitch: 60, start: 0, duration: 0.5, velocity: 0.2 }] })).at({ bar: 1 });
  await registerVst3Plugin(project, ${JSON.stringify({ bundlePath: resolve(gainPath), classId: "56455354494741494e30303030303031", allowPlugins: "any" })}, { hostPath: ${JSON.stringify(helperPath)} });
  return project;
}
`,
  );
  document = await ProjectDocument.open({ entry });
  assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
  await document.vst3Command(0, { kind: "scan" });
  assert.ok(document.view.vst3Bundles.includes(resolve(synthPath)), "system scan lists the installed fixture");
  assert.ok(document.view.vst3Bundles.includes(resolve(gainPath)));
  assert.equal(document.view.revision, 0, "directory discovery does not edit the source");
  const original = document.frame.snapshot.channels[0].instrument.pluginId;
  await document.vst3Command(0, { kind: "addBundle", bundlePath: resolve(synthPath) });
  const handle = document.view.plugins.find((plugin) => plugin.vst3?.bundlePath === resolve(synthPath)).handle;
  await document.assignPlugin(0, {
    plugin: handle,
    owner: document.frame.snapshot.channels[0].id,
    target: "instrument",
  });
  assert.equal(document.frame.snapshot.channels[0].instrument.pluginId, "vst3.564553544953594e5448303030303031");
  assert.equal(document.frame.vst3Plugins.length, 2);
  assert.ok(document.frame.snapshot.channels[0].instrument.state);
  await document.refreshPlugins(document.view.revision);
  const handles = document.view.plugins.map((p) => p.handle);
  assert.equal(new Set(handles).size, handles.length, "catalog refresh must not duplicate handles");
  const selected = document.view.plugins.find((p) => p.usages.some((u) => u.kind === "instrument"));
  assert.ok(selected.parameters.length > 0, "refresh must preserve VST3 parameter metadata");
  await document.undo(document.view.revision);
  assert.equal(document.frame.snapshot.channels[0].instrument.pluginId, original);
  assert.equal(document.frame.vst3Plugins.length, 1);
  await document.redo(document.view.revision);
  assert.equal(document.frame.vst3Plugins.length, 2);
  await document.save(document.view.revision);
  const saved = await readFile(entry, "utf8");
  assert.ok(saved.includes("withVst3Registration"));
  assert.ok(saved.includes("stateBase64"));
  document.close();
  document = await ProjectDocument.open({ entry });
  assert.equal(document.view.status, "ready", document.view.diagnostic?.message);
  assert.equal(document.frame.vst3Plugins.length, 2);
  const project = Project.fromSnapshot(document.frame.snapshot);
  for (const registration of document.frame.vst3Plugins) project.registerVst3(registration);
  const path = join(root, "instrument.wav");
  const report = await project.renderWav({ path, bitDepth: "float32", dither: "none" });
  assert.ok(report.files[0].peakDbfs > -60, "MIDI notes must produce instrument audio");
  const samples = await pcm(path);
  const early = Math.max(...samples.slice(2048, 12000).map(Math.abs));
  const late = Math.max(...samples.slice(-12000).map(Math.abs));
  assert.ok(early > 0.001);
  assert.ok(late < early * 0.01, "note-off must release the instrument before the final silence");
  if (process.env.OXITONE_VST3_VIEWER) {
    const image = resolve("target/vst3-daw-preview.png");
    const result = spawnSync(
      process.execPath,
      [resolve("packages/cli/dist/index.js"), "daw", entry, "--viewer", resolve(process.env.OXITONE_VST3_VIEWER)],
      {
        encoding: "utf8",
        timeout: 45_000,
        env: {
          ...process.env,
          OXITONE_PREVIEW_SIMULATED: "1",
          OXITONE_PREVIEW_CAPTURE: image,
          OXITONE_PREVIEW_CAPTURE_BUILTIN: "instrument",
          OXITONE_PREVIEW_APPEARANCE: "dark",
        },
      },
    );
    const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
    assert.equal(result.status, 0, output || String(result.error));
    assert.ok(output.includes("Preview capture saved"), output);
    console.log(`Native VST3 source project and generic instrument panel captured: ${image}`);
  }
  console.log(
    "VST3 DAW smoke passed: actual instrument assignment, registration/state source writeback, Undo/Redo/Save/reopen, MIDI note-on/off and native project WAV",
  );
} finally {
  document?.close();
  await rm(root, { recursive: true, force: true });
}
