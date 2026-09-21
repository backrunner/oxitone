import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { inspectVst3Plugin } from "../packages/vst3/dist/index.js";
import { vst3ControlResponseSchema } from "../packages/protocol/dist/index.js";

const bundlePath = process.env.OXITONE_VST3_FIXTURE;
assert(bundlePath, "Set OXITONE_VST3_FIXTURE to a local VestiGain.vst3 bundle");
function run(program, args) {
  const result = spawnSync(program, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${program} failed`);
}
run("cargo", [
  "build",
  "--locked",
  "--release",
  "-p",
  "oxitone-vst3-host",
  "--features",
  "host,stream",
  "--bin",
  "oxitone-vst3-host",
  "--example",
  "vst3-control-probe",
]);
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-control-"));
try {
  const hostPath = resolve("target/release/oxitone-vst3-host");
  const source = { bundlePath: resolve(bundlePath), classId: "56455354494741494e30303030303031", allowPlugins: "any" };
  const info = await inspectVst3Plugin(source, { hostPath });
  const startPath = join(root, "start.json");
  await writeFile(
    startPath,
    JSON.stringify({
      streamProtocolVersion: 11,
      source: { ...source, expectedHash: info.sha256 },
      options: {
        sampleRate: 48000,
        blockSize: 128,
        tempo: 120,
        timeSignature: [4, 4],
        configuration: info.configuration,
        parameters: {},
      },
    }),
  );
  const reportPath = join(root, "report.json");
  run(resolve("target/release/examples/vst3-control-probe"), [
    hostPath,
    startPath,
    reportPath,
    ...process.argv.slice(2),
  ]);
  const report = JSON.parse(await readFile(reportPath, "utf8"));
  vst3ControlResponseSchema.parse({
    controlProtocolVersion: 1,
    ok: true,
    state: { editorOpen: false, nextSequence: report.blocks, info: report.captured },
  });
  delete report.captured;
  Object.assign(report, { cpu: cpus()[0]?.model, os: `${process.platform} ${release()}`, arch: process.arch });
  if (process.env.OXITONE_VST3_CONTROL_REPORT)
    await writeFile(resolve(process.env.OXITONE_VST3_CONTROL_REPORT), `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    JSON.stringify({
      blocks: report.blocks,
      editor: report.nativeEditorAttachDetach,
      controlP95Ms: report.control.p95Ms,
      controlP99Ms: report.control.p99Ms,
      audioP95Ms: report.audioRoundtrip.p95Ms,
      audioP99Ms: report.audioRoundtrip.p99Ms,
    }),
  );
} finally {
  await rm(root, { recursive: true, force: true });
}
