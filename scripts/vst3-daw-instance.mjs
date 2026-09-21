// Current Preview instance -> Document source transaction -> Undo/Redo/Save/reopen.
import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { Project } from "../packages/core/dist/index.js";
import { startLiveDaw } from "./vst3-daw-runtime.mjs";
import { pcm, compare } from "./vst3-project-audio.mjs";

export async function verifyLiveDaw(
  source,
  metadata,
  host,
  { parameterId, initial, changed, playback = false, editor = false, gain },
) {
  // Keep Unix socket paths below macOS sockaddr_un capacity.
  const root = await mkdtemp("/tmp/oxi-live-daw-");
  let daw;
  const results = {};
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(resolve("packages/core"), join(root, "node_modules/@oxitone/core"), "dir");
    const entry = join(root, "song.ts");
    const preset = {
      pluginId: `vst3.${source.classId.toLowerCase()}`,
      pluginVersion: `0.0.0+${metadata.sha256}`,
      parameters: { [parameterId]: initial },
      mix: 1,
      bypass: false,
    };
    const registration = { registrationVersion: 1, source, helperPath: host.hostPath, metadata };
    await writeFile(join(root, "registration.json"), JSON.stringify(registration));
    const original = `import { Project, Pattern } from '@oxitone/core';
import registration from './registration.json';
const preset = { ...${JSON.stringify(preset)}, state: registration.metadata.configuration };
const project = new Project({ seed: 281 });
project.registerVst3(registration);
const channel = project.addChannel();
channel.addEffect(preset);
channel.addEffect(preset);
project.addTrack().use(channel).add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 0.5, velocity: 0.2 }] })).at({ bar: 1 });
export default project;
`;
    await writeFile(entry, original);
    daw = await startLiveDaw(entry, root);
    const effects = () => daw.frame.snapshot.channels[0].effectChain;
    const first = effects()[0].instanceId,
      second = effects()[1].instanceId;
    const before = structuredClone(daw.frame.snapshot);
    const signal = new AbortController().signal;
    const inventory = () => daw.runtime.inventory(daw.frame.snapshot.revision, signal);
    const control = async (id, command) => {
      const current = await inventory();
      return daw.runtime.control(
        daw.frame.snapshot.revision,
        {
          instanceControlVersion: 1,
          graphGeneration: current.graphGeneration,
          instanceId: id,
          command,
          timeoutMs: 5000,
        },
        signal,
      );
    };
    const site = () => {
      const site = daw.view.configurationSites.find(
        (site) => site.scope === "reference" && site.usages.length === 1 && site.usages[0].handle === first,
      );
      assert.ok(site, "selected instance needs one writable source boundary");
      return { site: site.handle, usage: first };
    };
    const capturedValue = async (id) =>
      (await control(id, { kind: "capture" })).state.info.configuration.parameters[String(parameterId)];
    const render = async (name) => {
      const project = Project.fromSnapshot(daw.frame.snapshot);
      for (const item of daw.frame.vst3Plugins) project.registerVst3(item);
      const path = join(root, `${name}.wav`);
      await project.renderWav({ path, bitDepth: "float32", dither: "none" });
      return pcm(path);
    };
    const baseline = gain === undefined ? undefined : await render("original");
    if (editor) {
      await daw.request({ kind: "vst3", command: { kind: "controlInstance", ...site(), action: "openEditor" } });
      assert.equal((await control(first, { kind: "poll" })).state.editorOpen, true);
    }
    if (playback) {
      daw.transport({ command: "play", frame: "0", loopRegion: { startFrame: "0", endFrame: "12000" } });
      await daw.until(() => daw.state.playing && BigInt(daw.state.audibleFrame) >= 256n);
    }
    const generation = (await inventory()).graphGeneration;
    await control(first, { kind: "setParameter", parameterId, value: changed });
    assert.equal(await capturedValue(first), changed);
    assert.equal(await capturedValue(second), initial);
    assert.deepEqual(daw.frame.snapshot, before);
    assert.equal(daw.view.revision, 0);
    assert.equal(await readFile(entry, "utf8"), original);
    if (editor) {
      await daw.request({ kind: "vst3", command: { kind: "controlInstance", ...site(), action: "closeEditor" } });
      assert.equal((await control(first, { kind: "poll" })).state.editorOpen, false);
      assert.equal(await capturedValue(first), changed, "closing editor preserves auditioned processor state");
    }
    assert.equal((await inventory()).graphGeneration, generation, "live controls do not replace the graph");
    await daw.request({ kind: "vst3", command: { kind: "captureInstance", ...site() } });
    assert.equal(daw.view.revision, 1);
    assert.equal(daw.view.modified, true);
    assert.equal(effects()[0].parameters[String(parameterId)], changed);
    assert.equal(effects()[0].mix, 1);
    assert.equal(effects()[0].bypass, false);
    assert.equal(effects()[0].instanceId, first);
    assert.deepEqual(effects()[1], before.channels[0].effectChain[1]);
    assert.equal(await readFile(entry, "utf8"), original);
    await daw.until(async () => (await inventory()).state === "active");
    assert.equal(await capturedValue(first), changed);
    await assert.rejects(
      daw.runtime.control(
        daw.frame.snapshot.revision,
        { instanceControlVersion: 1, graphGeneration: generation, instanceId: first, command: { kind: "poll" } },
        signal,
      ),
      { code: "PluginTaskConflict" },
    );
    await daw.request({ kind: "vst3", command: { kind: "captureInstance", ...site() } });
    assert.equal(daw.view.revision, 1, "accepting identical state does not add an Undo entry");
    if (playback) {
      results.playing = structuredClone(daw.state);
      assert.equal(daw.state.playing, true);
      assert.equal(daw.state.pluginFaults, 0);
      daw.transport({ command: "pause" });
      await daw.until(() => !daw.state.playing);
    }
    await daw.request({ kind: "undo" });
    assert.equal(effects()[0].parameters[String(parameterId)], initial);
    await daw.until(async () => (await inventory()).state === "active");
    assert.equal(await capturedValue(first), initial);
    await daw.request({ kind: "redo" });
    assert.equal(effects()[0].parameters[String(parameterId)], changed);
    await daw.request({ kind: "save" });
    const saved = await readFile(entry, "utf8");
    assert.equal(saved.match(/withState\(/g).length, 1);
    assert.equal(saved.match(/replaceParameters\(/g).length, 1);
    assert.equal(daw.view.modified, false);
    await daw.close();
    daw = await startLiveDaw(entry, root);
    assert.equal(effects()[0].instanceId, first);
    assert.equal(await capturedValue(first), changed);
    assert.equal(await capturedValue(second), initial);
    if (baseline) results.acceptedPcm = compare(await render("restored"), baseline, gain, "DAW accepted live state");
    results.checks = [
      "actualPreviewInstance",
      "independentSharedPresetUses",
      "sourceUnchangedByAudition",
      "captureTransaction",
      "identicalCaptureIsNoop",
      "stableInstanceIds",
      "hostMixBypassPreserved",
      "retiredGraphRejected",
      "undoRestoresLiveState",
      "redo",
      "saveReopenRestoresLiveState",
    ];
    if (editor) results.checks.push("openCloseCurrentNativeEditor");
    return results;
  } finally {
    await daw?.close();
    await rm(root, { recursive: true, force: true });
  }
}
