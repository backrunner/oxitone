// Real dylibs + native GPUI windows: incomplete source, runtime/native failure and panel recovery.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const temporary = mkdtempSync(join(root, "target/panel-watch-"));
const entry = join(temporary, "project.ts");
const layoutFile = join(temporary, "panel.ts");
const source = (mode = "valid") => `import createProject from "../../examples/drum-machine/src/preview.ts";
import { panel } from "./panel.js";
export default () => {
  ${mode === "runtime" ? 'throw new Error("Panel smoke runtime failure");' : ""}
  const p = createProject();
  const channel = p.channels[0];
  channel.instrument = { ...channel.instrument, pluginId: ${mode === "native" ? '"missing.instrument"' : '"example.drums"'},
    parameters: { ...channel.instrument.parameters, volume: ${mode === "recovered" ? 0.42 : 0.85} } };
  channel.effectChain = channel.effectChain.map(e => ({...e, mix: ${mode === "recovered" ? 0.37 : 0.8}}));
  return p.registerPluginUi(panel);
};`;
const layout = (title, parameter = "volume") => `import { drumPanel } from "../../examples/drum-machine/src/plugin-panels.ts";
export const panel = { ...drumPanel, title: ${JSON.stringify(title)}, pages: [{ id: "kit", title: "Kit", groups: [
 {id:"kit",title:"Kit shaping",columns:2,controls:[{kind:"knob",parameter:"decay"},{kind:"knob",parameter:${JSON.stringify(parameter)}}]}] }] };`;
writeFileSync(layoutFile, layout("Circuit Drums"));
writeFileSync(entry, source());
const viewer = resolve(root, process.argv[2] ?? "target/release/Oxitone Preview.app");
const child = spawn(process.execPath, [join(root, "packages/cli/dist/index.js"), "preview", entry, "--viewer", viewer], {
  cwd: root, env: { ...process.env, OXITONE_PREVIEW_CAPTURE: join(root, "target/plugin-panels-watch.png"),
    OXITONE_PREVIEW_APPEARANCE: "dark", OXITONE_PREVIEW_CAPTURE_PLUGIN: "instrument",
    OXITONE_PREVIEW_CAPTURE_WATCH: "1", OXITONE_PREVIEW_CAPTURE_REVISION: "4" },
  stdio: ["ignore", "pipe", "pipe"],
});
let stage = 0, log = "", pending = "", failure;
const receive = (chunk) => {
  process.stdout.write(chunk); log += chunk; pending += chunk;
  const lines = pending.split("\n"); pending = lines.pop();
  try {
    for (const line of lines) {
      if (stage === 0 && line.includes("Preview plugin windows opened")) {
        stage = 1; writeFileSync(entry, "export default = ;");
      }
      if (!line.startsWith("Preview panel-watch state ")) continue;
      const state = JSON.parse(line.slice("Preview panel-watch state ".length));
      if (state.windows.length < 2) continue;
      const drums = state.windows.find(w => w.pluginId === "example.drums");
      if (!drums) continue;
      if (stage <= 3 && state.error) {
        assert.equal(state.revision, 1);
        assert.ok(state.windows.every(w => w.revision === 1));
        assert.equal(drums.source.parameters.volume, 0.85);
        assert.equal(state.windows.find(w => w.pluginId === "fixture.gain").source.mix, 0.8);
        if (!state.windows.every(w => w.sync.includes("Last good"))) continue;
      }
      if (stage === 1 && state.error?.includes("PreviewBuildFailed") && drums.sync.includes("Last good")) {
        stage = 2; writeFileSync(entry, source("runtime"));
      } else if (stage === 2 && state.error?.includes("Panel smoke runtime failure")) {
        stage = 3; writeFileSync(entry, source("native"));
      } else if (stage === 3 && state.error?.includes("missing.instrument")) {
        stage = 4; writeFileSync(layoutFile, layout("Invalid panel", "unknown")); writeFileSync(entry, source("recovered"));
      } else if (stage === 4 && state.revision === 3 && state.windows.every(w => w.revision === 3)) {
        assert.equal(drums.title, "Circuit Drums");
        assert.ok(drums.uiError.includes("PluginUiInvalid"));
        assert.equal(drums.source.parameters.volume, 0.42);
        assert.equal(state.windows.find(w => w.pluginId === "fixture.gain").source.mix, 0.37);
        stage = 5; writeFileSync(layoutFile, layout("Circuit Drums II"));
      } else if (stage === 5 && state.revision === 4 && state.windows.every(w => w.revision === 4)) {
        assert.equal(drums.title, "Circuit Drums II"); assert.equal(drums.uiError, null);
        assert.ok(state.windows.every(w => w.sync === "Synced"));
        stage = 6;
      }
    }
  } catch (error) { failure = error; child.kill("SIGTERM"); }
};
child.stdout.on("data", receive); child.stderr.on("data", receive);
const timer = setTimeout(() => child.kill("SIGTERM"), 55_000);
try {
  const [code, signal] = await new Promise((done, reject) => {
    child.once("error", reject); child.once("exit", (code, signal) => done([code, signal]));
  });
  if (failure) throw failure;
  assert.equal(signal, null); assert.equal(code, 0); assert.equal(stage, 6, "all failure/recovery stages completed");
  assert.ok(log.includes("Preview plugin-window smoke passed")); assert.ok(log.includes("Preview capture saved"));
  console.log("Native custom-panel watch smoke passed");
} finally {
  clearTimeout(timer);
  if (child.exitCode === null && child.signalCode === null) child.kill("SIGTERM");
  rmSync(temporary, { recursive: true, force: true });
}
