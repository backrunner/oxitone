// Build only on the matching native CI runner. Installed packages never compile or download.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const arch = process.argv[2];
assert.ok(["arm64", "x64"].includes(arch), "expected arm64 or x64");
const check = process.argv.includes("--check");
if (process.platform !== "darwin" || process.arch !== arch) {
  if (check) throw new Error(`Pack @oxitone/vst3-host-darwin-${arch} on its matching macOS runner`);
  console.log(`Skipping VST3 helper for darwin-${arch} on ${process.platform}-${process.arch}`);
  process.exit(0);
}
const packageRoot = join(root, "packages", `vst3-host-darwin-${arch}`);
const output = join(packageRoot, "bin", "oxitone-vst3-host");
if (!check) {
  execFileSync(
    "cargo",
    [
      "build",
      "--release",
      "--locked",
      "-p",
      "oxitone-vst3-host",
      "--features",
      "host,stream",
      "--bin",
      "oxitone-vst3-host",
    ],
    { cwd: root, stdio: "inherit" },
  );
  mkdirSync(join(packageRoot, "bin"), { recursive: true });
  const temporary = `${output}.${process.pid}.tmp`;
  try {
    copyFileSync(join(root, "target", "release", "oxitone-vst3-host"), temporary);
    chmodSync(temporary, 0o755);
    renameSync(temporary, output);
  } finally {
    rmSync(temporary, { force: true });
  }
  const licenses = join(packageRoot, "LICENSES");
  mkdirSync(licenses, { recursive: true });
  copyFileSync(join(root, "LICENSE"), join(licenses, "Oxitone-MPL-2.0.txt"));
  copyFileSync(join(root, "vendor", "vst3-host", "LICENSE"), join(licenses, "vst3-host-MIT.txt"));
  copyFileSync(join(root, "vendor", "vst3-host", "OXITONE.md"), join(licenses, "vst3-host-modifications.md"));
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      [
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--features",
        "oxitone-vst3-host/host,oxitone-vst3-host/stream",
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
    ),
  );
  const host = metadata.packages.find((pkg) => pkg.name === "oxitone-vst3-host");
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const reachable = new Set();
  const visit = (id) => {
    if (reachable.has(id)) return;
    reachable.add(id);
    for (const dep of nodes.get(id)?.deps ?? []) {
      if (dep.dep_kinds.some((kind) => kind.kind !== "dev")) visit(dep.pkg);
    }
  };
  visit(host.id);
  const notices = metadata.packages
    .filter((pkg) => reachable.has(pkg.id))
    .map((pkg) => ({ name: pkg.name, version: pkg.version, license: pkg.license, repository: pkg.repository }));
  for (const pkg of metadata.packages.filter((pkg) => reachable.has(pkg.id))) {
    const directory = dirname(pkg.manifest_path);
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (!entry.isFile() || !/^(license|copying|notice)([.-]|$)/i.test(entry.name)) continue;
      const destination = join(licenses, `${pkg.name}-${pkg.version}`);
      mkdirSync(destination, { recursive: true });
      copyFileSync(join(directory, entry.name), join(destination, entry.name));
    }
  }
  writeFileSync(join(licenses, "dependencies.json"), `${JSON.stringify(notices, null, 2)}\n`);
}
assert.ok(statSync(output).mode & 0o111, "helper must be executable");
const machine = execFileSync("lipo", ["-archs", output], { encoding: "utf8" }).trim();
assert.equal(machine, arch === "arm64" ? "arm64" : "x86_64");
assert.ok(readFileSync(join(packageRoot, "LICENSES", "dependencies.json")).length > 0);
console.log(`VST3 helper package ready: darwin-${arch}`);
