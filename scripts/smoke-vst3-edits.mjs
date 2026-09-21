// Real IComponentHandler callbacks, native packet clock and SDK controls. No audio device.
import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { verifyEditSdk } from "./vst3-edit-sdk.mjs";
import { verifyRecordingSdk } from "./vst3-recording-sdk.mjs";

function run(program, args, capture = false) {
  const result = spawnSync(program, args, {
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${program} failed (${result.status})`);
  return result.stdout;
}
if (process.platform !== "darwin") throw new Error("VST3 conformance currently requires macOS");
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
  "vst3-edit-probe",
  "--example",
  "vst3-recording-probe",
]);
run("cargo", [
  "build",
  "--release",
  "--manifest-path",
  "crates/vst3-host/tests/fixtures/transport-plugin/Cargo.toml",
  "--target-dir",
  "target/vst3-transport-fixture",
  "--locked",
]);
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-edits-"));
try {
  const bundlePath = join(root, "Transport.vst3");
  await mkdir(join(bundlePath, "Contents", "MacOS"), { recursive: true });
  await copyFile(
    "target/vst3-transport-fixture/release/liboxitone_vst3_transport_fixture.dylib",
    join(bundlePath, "Contents", "MacOS", "Transport"),
  );
  await writeFile(
    join(bundlePath, "Contents", "Info.plist"),
    `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Transport</string><key>CFBundleIdentifier</key><string>dev.oxitone.transport-fixture</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>`,
  );
  const source = { bundlePath, classId: "6E33225254224A00AA69301AF318797D", allowPlugins: "any" };
  const hostPath = resolve("target/release/oxitone-vst3-host");
  const start = join(root, "start.json");
  await writeFile(
    start,
    JSON.stringify({
      streamProtocolVersion: 11,
      source,
      options: { sampleRate: 48000, blockSize: 128, parameters: {}, tempo: 120, timeSignature: [4, 4] },
    }),
  );
  const native = JSON.parse(run("target/release/examples/vst3-edit-probe", [hostPath, start], true));
  const recording = JSON.parse(
    run(
      "target/release/examples/vst3-recording-probe",
      [hostPath, start, ...(process.argv.includes("--benchmark") ? ["--benchmark"] : [])],
      true,
    ),
  );
  const sdk = await verifyEditSdk(source, hostPath);
  const recordingSdk = await verifyRecordingSdk(source, hostPath, root);
  const report = resolve(process.env.OXITONE_VST3_EDITS_REPORT ?? "target/vst3-edits.json");
  await mkdir(dirname(report), { recursive: true });
  await writeFile(
    report,
    `${JSON.stringify({ date: new Date().toISOString(), cpu: cpus()[0].model, os: `${process.platform} ${release()}`, profile: "release", sampleRate: 48000, blockSize: 128, device: "simulated stereo sink (SDK); no device (native)", callbackP95: null, callbackP99: null, native, recording, sdk, recordingSdk }, null, 2)}\n`,
  );
  console.log(`VST3 gesture conformance passed: ${report}`);
} finally {
  await rm(root, { recursive: true, force: true });
}
