import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";

function run(program, args) {
  const result = spawnSync(program, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${program} failed`);
}
if (process.platform !== "darwin") throw new Error("VST3 restart conformance requires macOS");
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
  "vst3-restart-probe",
]);
run("cargo", [
  "build",
  "--release",
  "--locked",
  "--manifest-path",
  "crates/vst3-host/tests/fixtures/transport-plugin/Cargo.toml",
  "--target-dir",
  "target/vst3-transport-fixture",
]);
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-restart-"));
try {
  const bundlePath = join(root, "Restart.vst3");
  await mkdir(join(bundlePath, "Contents", "MacOS"), { recursive: true });
  await copyFile(
    "target/vst3-transport-fixture/release/liboxitone_vst3_transport_fixture.dylib",
    join(bundlePath, "Contents", "MacOS", "Restart"),
  );
  await writeFile(
    join(bundlePath, "Contents", "Info.plist"),
    `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Restart</string><key>CFBundleIdentifier</key><string>dev.oxitone.restart-fixture</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>`,
  );
  const source = { bundlePath, classId: "6E33225254224A00AA69301AF3187984", allowPlugins: "any" };
  const host = { hostPath: resolve("target/release/oxitone-vst3-host") };
  const start = join(root, "start.json");
  const nativeReport = join(root, "native.json");
  await writeFile(
    start,
    JSON.stringify({
      streamProtocolVersion: 11,
      source,
      options: { sampleRate: 48000, blockSize: 128, parameters: {}, tempo: 120, timeSignature: [4, 4] },
    }),
  );
  run(resolve("target/release/examples/vst3-restart-probe"), [host.hostPath, start, nativeReport]);
  const report = JSON.parse(await readFile(nativeReport, "utf8"));
  delete report.captured;
  if (!process.argv.includes("--host-only")) {
    const { verifyRestartGraph } = await import("./vst3-restart-graph.mjs");
    report.graph = await verifyRestartGraph({ ...source, classId: "6E33225254224A00AA69301AF3187985" }, host, root);
    const { Project } = await import("../packages/core/dist/index.js");
    const { registerVst3Plugin, vst3Config } = await import("../packages/vst3/dist/index.js");
    const project = new Project();
    const registration = await registerVst3Plugin(project, source, host);
    project.master.inserts = [vst3Config(registration), vst3Config(registration)];
    const session = await project.compile({ allowPlugins: "any", audioBackend: "simulated" });
    try {
      const inventory = session.vst3Instances();
      const target = { graphGeneration: inventory.graphGeneration, instanceId: project.master.inserts[0].instanceId };
      const other = { ...target, instanceId: project.master.inserts[1].instanceId };
      const changed = await session.controlVst3Instance(target, { kind: "setParameter", parameterId: 0, value: 1 / 3 });
      assert.equal(changed.state.restart.latencyFrames, 64);
      const captured = await session.controlVst3Instance(target, { kind: "capture" });
      assert.equal(captured.state.info.configuration.parameters["0"], 1 / 3);
      assert.equal(captured.state.info.configuration.parameters["7"], 0.5);
      const untouched = await session.controlVst3Instance(other, { kind: "capture" });
      assert.equal(untouched.state.restart, undefined);
      assert.equal(untouched.state.info.configuration.parameters["0"], 0);
      await assert.rejects(session.controlVst3Instance(target, { kind: "setParameter", parameterId: 0, value: 0 }), {
        code: "PluginRestartRequired",
      });
      await session.seek({ frame: 1 });
      await session.play();
      await new Promise((resolve) => setTimeout(resolve, 150));
      await session.pause();
      const after = await session.controlVst3Instance(target, { kind: "capture" });
      assert.deepEqual(after.state.info, captured.state.info);
      assert.deepEqual(after.state.restart, captured.state.restart);
      report.checks.push("sdkNativeFrozenCapture", "otherInstanceUnchanged", "graphSeekPlayPreservesFrozenState");
    } finally {
      await session.dispose();
    }
  }
  report.date = new Date().toISOString();
  report.cpu = cpus()[0].model;
  report.os = `${process.platform} ${release()}`;
  report.sdkNative = !process.argv.includes("--host-only");
  report.guiVisualAcceptance = false;
  const output = resolve(process.env.OXITONE_VST3_RESTART_REPORT ?? "target/vst3-restart.json");
  await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`VST3 restart conformance passed: ${output}`);
} finally {
  await rm(root, { recursive: true, force: true });
}
