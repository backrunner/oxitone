import { execFile } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, release } from "node:os";
import { resolve } from "node:path";
import { promisify } from "node:util";

// Serialize native windows; no test opens an audio device or changes system appearance.
const output = resolve("target/ui-review");
await mkdir(output, { recursive: true });
const scenarios = [
  ["controls-dark", "1440x920", "dark", { CONTROLS: "1" }, "Controls smoke passed:"],
  ["controls-light-small", "1060x720", "light", { CONTROLS: "1" }, "Controls smoke passed:"],
  ["library-dark", "1440x920", "dark", { MANAGER: "1", LIBRARY: "1" }, "Plugin library smoke passed:"],
  ["library-light-small", "1060x720", "light", { MANAGER: "1", LIBRARY: "1" }, "Plugin library smoke passed:"],
  ["library-details-small", "1060x720", "light", { MANAGER: "1", LIBRARY: "details", CONFIGURATION: "1" }, "Plugin library smoke passed:"],
  ["windows-dark", "1440x920", "dark", { PATTERNS: "1", WINDOWS: "1" }, "Internal windows smoke passed:"],
  ["windows-light-small", "1060x720", "light", { PATTERNS: "1", WINDOWS: "1" }, "Internal windows smoke passed:"],
  ["piano-dark", "1440x920", "dark", { EDITING: "1", WORKSPACE: "piano" }, "Workspace smoke passed:"],
  ["automation-light-small", "1060x720", "light", { AUTOMATION: "1", EDITING: "1" }, "Editing smoke passed:"],
  ["effects-light-small", "1060x720", "light", { CONFIGURATION: "1" }, "Configuration smoke passed:"],
  ["effects-dark", "1440x920", "dark", { CONFIGURATION: "1" }, "Configuration smoke passed:"],
  ["close-dark-small", "1060x720", "dark", { UI_REVIEW: "1" }, "UI review smoke passed:"],
  ["close-light", "1440x920", "light", { UI_REVIEW: "1" }, "UI review smoke passed:"],
];
const results = [];
const record = status => writeFile(`${output}/results.json`, JSON.stringify({
  status, startedAt, cpu: cpus()[0]?.model, os: `darwin ${release()}`, node: process.version,
  audioSink: "simulated", physicalDevice: null,
  input: "GPUI keyboard dispatch and targeted NSEvents; not physical-device acceptance", results,
}, null, 2) + "\n");
const startedAt = new Date().toISOString();
await record("running");
for (const [name, size, appearance, flags, expected] of scenarios) {
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith("OXITONE_PREVIEW_CAPTURE")));
  for (const [key, value] of Object.entries(flags)) env[`OXITONE_PREVIEW_CAPTURE_${key}`] = value;
  env.OXITONE_PREVIEW_CAPTURE_SIZE = size;
  env.OXITONE_PREVIEW_APPEARANCE = appearance;
  env.OXITONE_PREVIEW_CAPTURE_OUTPUT = `${output}/${name}.png`;
  try {
    await promisify(execFile)(process.execPath, ["scripts/smoke-daw.mjs"], { env, timeout: 170_000, maxBuffer: 2 * 1024 * 1024 });
    const log = await readFile(`${env.OXITONE_PREVIEW_CAPTURE_OUTPUT}.log`, "utf8");
    if (!log.includes(expected)) throw new Error(`Missing completion marker: ${expected}`);
    const png = await readFile(env.OXITONE_PREVIEW_CAPTURE_OUTPUT);
    if (png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") throw new Error("Missing PNG capture");
    results.push({ name, size, appearance, flags, passed: true });
    await record("running");
    console.log(`PASS ${name}`);
  } catch (error) {
    const log = `${output}/${name}.failure.log`;
    await writeFile(log, error.stderr ?? String(error));
    results.push({ name, size, appearance, flags, passed: false, log });
    console.error(`FAIL ${name}; details: ${log}`);
    process.exitCode = 1;
    break;
  }
}
await record(results.length === scenarios.length && results.every(result => result.passed) ? "passed" : "failed");
