// macOS GUI smoke using the real drum dylib, watch runner, and live detail windows.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.platform !== "darwin") throw new Error("The native detail-window smoke requires macOS");
const root = fileURLToPath(new URL("../", import.meta.url));
const output = join(root, "target");
mkdirSync(output, { recursive: true });
const temporary = mkdtempSync(join(output, "preview-details-"));
const entry = join(temporary, "project.ts");
const source = (volume) => `import createProject from "../../examples/drum-machine/src/preview.ts";
export default function () {
  const project = createProject();
  const channel = project.channels[0];
  channel.instrument = { ...channel.instrument, parameters: { ...channel.instrument.parameters, volume: ${volume} } };
  return project;
}
`;
writeFileSync(entry, source(0.85));
const screenshot = join(output, "plugin-details-watch.png");
const viewer = resolve(root, process.argv[2] ?? "target/release/Oxitone Preview.app");
const child = spawn(process.execPath, [join(root, "packages/cli/dist/index.js"), "preview", entry, "--viewer", viewer], {
  cwd: root,
  env: { ...process.env, OXITONE_PREVIEW_CAPTURE: screenshot, OXITONE_PREVIEW_APPEARANCE: "dark",
    OXITONE_PREVIEW_CAPTURE_PLUGIN: "instrument", OXITONE_PREVIEW_CAPTURE_REVISION: "2" },
  stdio: ["ignore", "pipe", "pipe"],
});
let log = "";
let changed = false;
const receive = (chunk) => {
  const text = chunk.toString();
  process.stdout.write(text);
  log += text;
  if (!changed && log.includes("Preview plugin windows opened")) {
    changed = true;
    writeFileSync(entry, source(0.42));
  }
};
child.stdout.on("data", receive);
child.stderr.on("data", receive);
const timer = setTimeout(() => child.kill("SIGTERM"), 25_000);
try {
  const [code, signal] = await new Promise((done, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => done([code, signal]));
  });
  assert.equal(signal, null, "viewer must exit normally after capture");
  assert.equal(code, 0);
  assert.ok(changed, "windows must open before the code edit");
  assert.ok(log.includes("Preview plugin-window smoke passed"));
  assert.ok(log.includes("Preview capture saved"));
  const states = [...log.matchAll(/Preview plugin-window state (\{[^\n]+\})/g)].map((m) => JSON.parse(m[1]));
  assert.ok(states.length >= 2, "instrument and effect windows remain open");
  assert.ok(states.every((state) => state.revision >= 2));
  assert.equal(states.find((state) => state.pluginId === "example.drums")?.parameters.volume, 0.42);
  console.log(`Plugin detail watch smoke passed: ${screenshot}`);
} finally {
  clearTimeout(timer);
  if (child.exitCode === null && child.signalCode === null) child.kill("SIGTERM");
  rmSync(temporary, { recursive: true, force: true });
}
