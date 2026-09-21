// Actual Preview capture -> source transaction -> independent PCM -> Undo/Redo/Save/reopen.
import assert from "node:assert/strict";
import { copyFile, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { cpus, release } from "node:os";
import { join, resolve, dirname } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { Project, createAutomationNamespace } from "../packages/core/dist/index.js";
import { inspectVst3Plugin } from "../packages/vst3/dist/index.js";
import { startLiveDaw } from "./vst3-daw-runtime.mjs";

const root = await mkdtemp("/tmp/oxi-rec-daw-");
let daw;
try {
  const bundlePath = join(root, "Recording.vst3");
  await mkdir(join(bundlePath, "Contents/MacOS"), { recursive: true });
  await copyFile(
    "target/vst3-transport-fixture/release/liboxitone_vst3_transport_fixture.dylib",
    join(bundlePath, "Contents/MacOS/Recording"),
  );
  await writeFile(
    join(bundlePath, "Contents/Info.plist"),
    `<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Recording</string><key>CFBundleIdentifier</key><string>dev.oxitone.recording-fixture</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>`,
  );
  const source = { bundlePath, classId: "6E33225254224A00AA69301AF3187983", allowPlugins: "any" };
  const helperPath = resolve("target/release/oxitone-vst3-host");
  const metadata = await inspectVst3Plugin(source, { hostPath: helperPath });
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(resolve("packages/core"), join(root, "node_modules/@oxitone/core"), "dir");
  await writeFile(
    join(root, "registration.json"),
    JSON.stringify({ registrationVersion: 1, source, helperPath, metadata }),
  );
  const entry = join(root, "song.ts");
  const original = `import { Project, Pattern, createAutomationNamespace } from '@oxitone/core';
import registration from './registration.json';
const project = new Project({ seed: 291 });
project.registerVst3(registration);
const channel = project.addChannel();
channel.addEffect({ pluginId: 'vst3.${source.classId.toLowerCase()}', pluginVersion: '0.0.0+${metadata.sha256}', parameters: { '0': 0.2 }, state: registration.metadata.configuration });
const automation = createAutomationNamespace();
const parameter = channel.effectInstances[0].param('0');
parameter.automate(automation.constant(0.2), { combine: 'replace' });
const local = parameter.automate(automation.constant(0.1), { combine: 'add', playback: 'playlist', loop: { lengthBeats: 0.25 } });
project.createAutomationClip(local, project.addTrack('Existing automation'), 0.125, 0.25);
project.addTrack('Notes').use(channel).add(new Pattern({ lengthBeats: 4, notes: [{ pitch: 60, start: 0, duration: 3, velocity: 0.2 }] })).at({ bar: 1 });
project.tracks.forEach((track) => { track.solo = true; });
export default project;
`;
  await writeFile(entry, original);
  daw = await startLiveDaw(entry, root);
  const before = structuredClone(daw.frame.snapshot);
  const instanceId = before.channels[0].effectChain[0].instanceId;
  const signal = new AbortController().signal;
  const inventory = () => daw.runtime.inventory(daw.frame.snapshot.revision, signal);
  const control = async (command) => {
    const current = await inventory();
    return daw.runtime.control(
      daw.frame.snapshot.revision,
      { instanceControlVersion: 1, graphGeneration: current.graphGeneration, instanceId, command },
      signal,
    );
  };
  const site = () => {
    const item = daw.view.configurationSites.find((s) => s.usages.length === 1 && s.usages[0].handle === instanceId);
    assert(item);
    return { site: item.handle, ...(item.scope === "reference" ? { usage: instanceId } : {}) };
  };
  const samples = new Map();
  const originalControl = daw.runtime.control.bind(daw.runtime);
  daw.runtime.control = async (...args) => {
    const response = await originalControl(...args);
    for (const event of response.state.edits?.events ?? [])
      if (event.kind === "sample") samples.set(event.sequence, event);
    return response;
  };
  const render = async (snapshot, name) => {
    const project = Project.fromSnapshot(snapshot);
    project.registerVst3({ registrationVersion: 1, source, helperPath, metadata });
    const path = join(root, `${name}.wav`);
    try {
      await project.renderWav({ path, bitDepth: "float32", dither: "none" });
    } finally {
      await project.session?.dispose();
    }
    return readFile(path);
  };
  const baseline = await render(before, "baseline");
  await daw.until(async () => (await inventory()).state === "active");
  await daw.request({ kind: "vst3", command: { kind: "startRecording", ...site(), mode: "write", parameterIds: [0] } });
  const recordingId = daw.view.vst3Recording.id;
  await daw.request({ kind: "save" });
  assert.equal(daw.view.vst3Recording.id, recordingId, "Save must preserve an unchanged recording revision");
  daw.transport({ command: "play", frame: "0", loopRegion: { startFrame: "0", endFrame: "12000" } });
  await daw.until(() => samples.size > 10);
  await control({ kind: "setParameter", parameterId: 99, value: 0.125 });
  await delay(120);
  daw.transport({ command: "seek", frame: "0" });
  await delay(100);
  const nativeInventory = daw.runtime.inventory.bind(daw.runtime);
  let rejectAcceptance = true;
  daw.runtime.inventory = async (...args) => {
    if (rejectAcceptance) {
      rejectAcceptance = false;
      throw new Error("Injected acceptance inventory failure");
    }
    return nativeInventory(...args);
  };
  await assert.rejects(daw.request({ kind: "vst3", command: { kind: "stopRecording", recordingId } }));
  assert.equal(daw.view.vst3Recording.status, "captured");
  assert.equal(daw.view.revision, 0);
  await daw.request({ kind: "vst3", command: { kind: "stopRecording", recordingId } });
  assert.equal(daw.view.vst3Recording, undefined);
  assert.equal(daw.view.revision, 1);
  const accepted = structuredClone(daw.frame.snapshot);
  assert.deepEqual(accepted.channels, before.channels);
  assert.deepEqual(
    accepted.automation.filter((lane) => !lane.priority),
    before.automation,
  );
  assert.equal(accepted.tracks.length, before.tracks.length + 1);
  assert.equal(await readFile(entry, "utf8"), original);
  const text = daw.view.files.find((file) => file.path.endsWith("song.ts")).text;
  assert(text.includes("recordingSource0"));
  const recordedLane = accepted.automation.find((lane) => lane.priority === 1);
  assert(
    daw.view.automationSites.some((site) => site.lanes.includes(recordedLane.id)),
    "recorded curves must remain source-editable",
  );
  assert(!text.includes(instanceId) && !text.includes(recordingId));
  assert(text.includes(original.slice(0, original.indexOf("export default"))));
  daw.transport({ command: "pause" });
  await daw.until(() => !daw.state.playing);
  const recorded = await render(accepted, "recorded");
  assert.notDeepEqual(recorded, baseline);
  // Independent oracle: original automation is 0.2 + 0.1 in [0.125, 0.375).
  // Flatten the native packet clock directly, without recordingRanges or Project.recordAutomation.
  const events = [...samples.values()].sort((a, b) => a.sequence - b.sequence);
  assert(events.some((event) => event.value === 0.75));
  assert(
    events.some((event, i) => i && event.position.transport.projectBeat < events[i - 1].position.transport.projectBeat),
  );
  const intervals = events.map((event) => ({
    start: event.position.transport.projectBeat,
    end: event.position.transport.projectBeat + event.frames / 24000,
    value: event.value,
  }));
  const edges = [
    ...new Set([
      0,
      0.125,
      0.375,
      ...intervals.flatMap((span) => [span.start, span.end]).map((beat) => Math.round(beat * 24000) / 24000),
    ]),
  ].sort((a, b) => a - b);
  const valueAt = (beat) =>
    intervals.reduce(
      (value, span) => (beat + 1e-10 >= span.start && beat < span.end - 1e-10 ? span.value : value),
      beat >= 0.125 && beat < 0.375 ? 0.3 : 0.2,
    );
  const reference = Project.fromSnapshot(before);
  for (const clip of reference.automationClips) reference.removeAutomationClip(clip);
  for (const lane of reference.automationLanes) reference.removeAutomationLane(lane);
  reference.addAutomationLane(
    { entityId: instanceId, parameterId: "0", scope: "plugin" },
    createAutomationNamespace().curve(edges.map((beat) => ({ beat, value: valueAt(beat), curve: { kind: "step" } }))),
  );
  assert.deepEqual(recorded, await render(reference.snapshot(), "reference"));
  const editable = daw.view.automationSites.find(
    (site) => site.scope === "definition" && site.lanes.includes(recordedLane.id),
  );
  assert(editable, "each recorded curve has one definition boundary");
  await daw.request({
    kind: "automationRange",
    site: editable.handle,
    edit: { start: 0, end: 0.1, points: [{ beat: 0, value: 0.4, curve: { kind: "step" } }] },
  });
  assert.notDeepEqual(await render(daw.frame.snapshot, "edited-curve"), recorded);
  await daw.request({ kind: "undo" });
  assert.deepEqual(await render(daw.frame.snapshot, "undo-curve"), recorded);
  await daw.request({ kind: "undo" });
  assert.deepEqual(await render(daw.frame.snapshot, "undo"), baseline);
  await daw.request({ kind: "redo" });
  assert.deepEqual(await render(daw.frame.snapshot, "redo"), recorded);
  await daw.request({ kind: "save" });
  const saved = await readFile(entry, "utf8");
  assert.equal(saved.match(/const recordingSource0/g).length, 1);
  await daw.close();
  daw = await startLiveDaw(entry, root);
  assert.deepEqual(await render(daw.frame.snapshot, "reopen"), recorded);
  await daw.until(async () => (await inventory()).state === "active");
  await daw.request({ kind: "vst3", command: { kind: "startRecording", ...site(), mode: "touch", parameterIds: [0] } });
  const emptyId = daw.view.vst3Recording.id;
  daw.transport({ command: "play", frame: "0" });
  await daw.until(() => daw.state.playing);
  await daw.request({ kind: "vst3", command: { kind: "stopRecording", recordingId: emptyId } });
  assert.equal(daw.view.revision, 0, "empty Touch take must not create a source revision");
  daw.transport({ command: "pause" });
  await daw.until(() => !daw.state.playing);
  await daw.request({ kind: "vst3", command: { kind: "startRecording", ...site(), mode: "write", parameterIds: [0] } });
  const cancelId = daw.view.vst3Recording.id;
  const pendingStop = daw.request({ kind: "vst3", command: { kind: "stopRecording", recordingId: cancelId } }).then(
    () => false,
    () => true,
  );
  await daw.until(() => daw.view.vst3Recording?.status === "stopping");
  await daw.request({ kind: "vst3", command: { kind: "cancelRecording", recordingId: cancelId } });
  assert.equal(await pendingStop, true);
  assert.equal(daw.view.vst3Recording, undefined);
  assert.equal(daw.view.revision, 0);
  assert.equal(await readFile(entry, "utf8"), saved);
  // Cancellation while startRecording is awaiting its native response must clean that exact capture.
  const liveControl = daw.runtime.control.bind(daw.runtime);
  let releaseStart;
  const gate = new Promise((resolve) => {
    releaseStart = resolve;
  });
  let startReturned = false;
  daw.runtime.control = async (...args) => {
    const response = await liveControl(...args);
    if (args[1].command.kind === "startRecording") {
      startReturned = true;
      await gate;
    }
    return response;
  };
  const pendingStart = daw
    .request({ kind: "vst3", command: { kind: "startRecording", ...site(), mode: "touch", parameterIds: [0] } })
    .then(
      () => false,
      () => true,
    );
  await daw.until(() => startReturned);
  const startingId = daw.view.vst3Recording.id;
  const cancelling = daw.request({ kind: "vst3", command: { kind: "cancelRecording", recordingId: startingId } });
  releaseStart();
  await cancelling;
  assert.equal(await pendingStart, true);
  assert.equal(daw.view.vst3Recording, undefined);
  daw.runtime.control = liveControl;
  // A fresh capture proves the late original capture was actually discarded.
  await daw.request({ kind: "vst3", command: { kind: "startRecording", ...site(), mode: "touch", parameterIds: [0] } });
  const staleId = daw.view.vst3Recording.id;
  const sourceFile = daw.view.files.find((file) => file.path.endsWith("song.ts"));
  await daw.request({
    kind: "code",
    fileName: sourceFile.path,
    text: `${sourceFile.text}\n// Source changed after recording began.\n`,
  });
  assert.equal(daw.view.vst3Recording.status, "failed");
  await daw.request({ kind: "vst3", command: { kind: "cancelRecording", recordingId: staleId } });
  assert.equal(daw.view.vst3Recording, undefined);
  assert.equal(await readFile(entry, "utf8"), saved);
  const report = resolve(process.env.OXITONE_VST3_RECORDING_DAW_REPORT ?? "target/vst3-recording-daw.json");
  await mkdir(dirname(report), { recursive: true });
  await writeFile(
    report,
    `${JSON.stringify({ date: new Date().toISOString(), cpu: cpus()[0].model, os: release(), engineProtocol: "1.5", device: "simulated", guiVisualAcceptance: false, nativeSamples: events.length, clips: accepted.automationClips.length - before.automationClips.length, wavBytes: recorded.length, checks: ["actualPreviewRecording", "loopAndSeek", "preservesExistingPlaylistAndCombine", "sourceTransaction", "retainsCompleteTakeOnAcceptanceFailure", "retryAcceptance", "independentNativePacketPcmExact", "editRecordedCurve", "undoCurvePcmExact", "undoPcmExact", "redoPcmExact", "saveReopenPcmExact", "emptyTouchNoop", "cancelPendingStop", "cancelPendingStart", "lateStartCaptureCleanup", "sourceChangeInvalidatesRecording"] }, null, 2)}\n`,
  );
  console.log(`VST3 DAW recording conformance passed: ${report}`);
} finally {
  await daw?.close();
  await rm(root, { recursive: true, force: true });
}
