import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { cpus, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";

function run(program, args) {
  const result = spawnSync(program, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${program} failed (${result.status})`);
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
  "vst3-transport-probe",
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
const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-transport-"));
try {
  const bundle = join(root, "Transport.vst3");
  await mkdir(join(bundle, "Contents", "MacOS"), { recursive: true });
  await copyFile(
    "target/vst3-transport-fixture/release/liboxitone_vst3_transport_fixture.dylib",
    join(bundle, "Contents", "MacOS", "Transport"),
  );
  await writeFile(
    join(bundle, "Contents", "Info.plist"),
    `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Transport</string><key>CFBundleIdentifier</key><string>dev.oxitone.transport-fixture</string><key>CFBundlePackageType</key><string>BNDL</string></dict></plist>`,
  );
  const start = join(root, "start.json");
  await writeFile(
    start,
    JSON.stringify({
      streamProtocolVersion: 11,
      source: { bundlePath: bundle, classId: "6E33225254224A00AA69301AF318797D", allowPlugins: "any" },
      options: { sampleRate: 48000, blockSize: 128, parameters: {}, tempo: 120, timeSignature: [4, 4] },
    }),
  );
  const report = resolve(process.env.OXITONE_VST3_TRANSPORT_REPORT ?? "target/vst3-transport.json");
  run("target/release/examples/vst3-transport-probe", [resolve("target/release/oxitone-vst3-host"), start, report]);
  const measured = JSON.parse(await readFile(report, "utf8"));
  Object.assign(measured, {
    date: new Date().toISOString(),
    cpu: cpus()[0].model,
    os: `${process.platform} ${release()}`,
    profile: "release",
  });
  await writeFile(report, `${JSON.stringify(measured, null, 2)}\n`);
  console.log(`Report: ${report}`);
} finally {
  await rm(root, { recursive: true, force: true });
}
