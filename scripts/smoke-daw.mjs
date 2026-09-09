import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { ProjectDocument } from "../packages/cli/dist/source/index.js";
import { dawPluginFixture } from "./daw-plugin-fixture.mjs";

const root = await mkdtemp(join(tmpdir(), "oxitone-daw-smoke-"));
const automation = !!process.env.OXITONE_PREVIEW_CAPTURE_AUTOMATION;
const configuration = !!process.env.OXITONE_PREVIEW_CAPTURE_CONFIGURATION;
const patterns = !!process.env.OXITONE_PREVIEW_CAPTURE_PATTERNS;
const output = resolve(process.env.OXITONE_PREVIEW_CAPTURE_OUTPUT ?? (process.env.OXITONE_PREVIEW_CAPTURE_UI_REVIEW ? "target/daw-ui-review.png" : configuration ? "target/daw-configuration.png" : automation ? "target/daw-automation.png" : patterns ? "target/daw-patterns.png" : process.env.OXITONE_PREVIEW_CAPTURE_MANAGER ? "target/daw-plugins.png" : "target/daw-edit.png"));
try {
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../packages/core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts");
  await writeFile(entry, "import { Project, chord, wavetable } from '@oxitone/core';\nconst phrase = chord(60, 'major');\nconst project = new Project({ name: 'Untitled' });\nconst keys = project.addChannel({ name: 'Keys', instrument: wavetable({ oscA: { wave: 'triangle' } }) });\nproject.addTrack('Shared phrase').use(keys).add(phrase).at({ bar: 1 });\nproject.addTrack('Second placement').use(keys).add(phrase).at({ bar: 2 });\nexport default project;\n");
  if (process.env.OXITONE_PREVIEW_CAPTURE_EDITING) await writeFile(entry, (await readFile(entry, "utf8")).replace("chord(60, 'major')", "chord(60, 'major', { lengthBeats: 8 })"));
  if (automation) await writeFile(entry, (await readFile(entry, "utf8"))
    .replace("{ Project, chord, wavetable }", "{ Project, chord, wavetable, createAutomationNamespace }")
    .replace("export default project;", "const motion = createAutomationNamespace().sine({ periodBeats: 4 });\nkeys.automate('level', motion); keys.automate('pan', motion);\nexport default project;"));
  const checkPluginFiles = configuration ? await dawPluginFixture(root, entry) : undefined;
  const result = await promisify(execFile)(process.execPath, ["packages/cli/dist/index.js", "daw", entry, "--viewer", resolve("target/debug/Oxitone Preview.app")], {
    env: { ...process.env, OXITONE_PREVIEW_CAPTURE: output, OXITONE_PREVIEW_CAPTURE_DAW: "1" }, timeout: process.env.OXITONE_PREVIEW_CAPTURE_EDITING || process.env.OXITONE_PREVIEW_CAPTURE_UI_REVIEW ? 150_000 : 45_000, maxBuffer: 1024 * 1024,
  });
  await writeFile(process.env.OXITONE_PREVIEW_CAPTURE_OUTPUT ? `${output}.log` : configuration ? "target/daw-configuration-smoke.log" : automation ? "target/daw-automation-smoke.log" : patterns ? "target/daw-patterns-smoke.log" : process.env.OXITONE_PREVIEW_CAPTURE_MANAGER ? "target/daw-manager-smoke.log" : "target/daw-smoke.log", result.stderr);
  if (!result.stderr.includes("DAW smoke passed:") || !result.stderr.includes("Preview capture saved")) throw new Error(result.stderr);
  if (process.env.OXITONE_PREVIEW_CAPTURE_UI_REVIEW && !result.stderr.includes("UI review smoke passed:")) throw new Error(result.stderr);
  if (automation && !result.stderr.includes("Automation smoke passed:")) throw new Error(result.stderr);
  if (configuration && !result.stderr.includes("Configuration smoke passed:")) throw new Error(result.stderr);
  if (patterns && !result.stderr.includes("Pattern browser smoke passed:")) throw new Error(result.stderr);
  if (process.env.OXITONE_PREVIEW_CAPTURE_EDITING && !result.stderr.includes("Editing smoke passed:")) throw new Error(result.stderr);
  if (process.env.OXITONE_PREVIEW_CAPTURE_EDITING && !automation && !result.stderr.includes("Piano smoke passed:")) throw new Error(result.stderr);
  if (process.env.OXITONE_PREVIEW_CAPTURE_CONTROLS && !result.stderr.includes("Controls smoke passed:")) throw new Error(result.stderr);
  await checkPluginFiles?.();
  const saved = await readFile(entry, "utf8");
  if (process.env.OXITONE_PREVIEW_CAPTURE_UI_REVIEW && !saved.includes("// UI close review")) throw new Error("Save & close did not publish the latest draft");
  if (!saved.includes(".edit(") || saved.includes("__oxitone_project_") || /requestId|sessionId/.test(saved)) throw new Error("invalid persisted source");
  const reopened = await ProjectDocument.open({ entry });
  try {
    const snapshot = reopened.frame?.snapshot;
    if (process.env.OXITONE_PREVIEW_CAPTURE_CONTROLS && (snapshot?.channels[0]?.level <= 1 || snapshot?.channels[0]?.pan !== .25 || !snapshot?.channels[0]?.mute || snapshot?.tempoMap[0]?.bpm !== 132)) throw new Error("DAW controls did not survive source reopen");
    if (process.env.OXITONE_PREVIEW_CAPTURE_CONTROLS && (!snapshot?.tracks[0]?.mute || !snapshot?.tracks[0]?.solo || snapshot?.tracks[1]?.mute || snapshot?.tracks[1]?.solo)) throw new Error("Track M/S did not survive source reopen independently");
    const selected = snapshot?.patterns.find(pattern => pattern.id === snapshot.patternClips[0]?.patternId);
    const shared = snapshot?.patterns.find(pattern => pattern.id === snapshot.patternClips[1]?.patternId);
    if (process.env.OXITONE_PREVIEW_CAPTURE_EDITING && !automation &&
      (selected?.lengthBeats.numerator / selected?.lengthBeats.denominator !== 21 ||
        !selected?.notes.some(note => note.start.numerator / note.start.denominator === 20) ||
        !selected?.notes.some(note => note.velocity > .9) || !selected?.notes.some(note => note.velocity < .4))) {
      throw new Error("velocity drawing and phrase extension did not survive source reopen");
    }
    if (reopened.view.status !== "ready" || !selected?.notes.some(note => note.pitch === 61 && note.start.numerator / note.start.denominator === .25)
      || !shared?.notes.some(note => note.pitch === 60 && note.start.numerator === 0)) {
      throw new Error("saved GPUI edit did not survive a fresh project process");
    }
    if (automation && (snapshot.automation[0]?.source.kind !== "replaceRange" || snapshot.automation[1]?.source.kind !== "wave")) throw new Error("automation range did not survive source reopen");
    if (configuration && (snapshot.channels[0]?.effectChain[0]?.parameters.gain !== .9 || snapshot.channels[1]?.effectChain[1]?.parameters.gain !== .4
      || snapshot.automation.at(-1)?.target.entityId !== snapshot.channels[0]?.effectChain[1]?.instanceId || /instanceId|ins_[a-f0-9]/.test(saved))) throw new Error("plugin configuration, order or typed binding did not survive source reopen");
  } finally { reopened.close(); }
  console.log(`DAW smoke passed; source saved and reopened; capture ${output}; simulated audio only`);
} finally { await rm(root, { recursive: true, force: true }); }
