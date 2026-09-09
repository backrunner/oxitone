import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { expect, it } from "vitest";
import { Project } from "@oxitone/core";
import type { PreviewSnapshotFrame } from "@oxitone/protocol";
import { discoverProjectPlugins } from "../src/source/plugin-discovery.js";
import { ProjectPluginCatalog } from "../src/source/project-plugins.js";
import { PluginLifecycle, type PluginTaskRunner } from "../src/source/plugin-lifecycle.js";

it("discovers unused multi-plugin packages without executing JS, validates only the selected native library, and exposes failures", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-plugin-catalog-"));
  try {
    const pkg = join(root, "node_modules/catalog-fixture"); await mkdir(pkg, { recursive: true });
    await writeFile(join(root, "package.json"), '{"dependencies":{"catalog-fixture":"1.2.3"}}');
    await writeFile(join(pkg, "package.json"), '{"name":"catalog-fixture","version":"1.2.3","type":"module","exports":"./index.js","oxitone":{"plugins":"plugins.json"}}');
    await writeFile(join(pkg, "index.js"), "throw new Error('catalog must not execute this package');");
    const repo = fileURLToPath(new URL("../../../", import.meta.url));
    const fixtures = join(repo, "crates/render/tests/fixtures");
    const manifest = JSON.parse(await readFile(join(fixtures, "gain.json"), "utf8"));
    const binary = join(pkg, process.platform === "darwin" ? "gain.dylib" : "gain.so");
    await promisify(execFile)("cc", ["-std=c11", "-shared", "-fPIC", "-O2", "-I", join(repo, "include"), join(fixtures, "gain.c"), "-o", binary]);
    const sha256 = createHash("sha256").update(await readFile(binary)).digest("hex");
    const platform = `${process.platform}-${process.arch}`;
    const plugin = { displayName: "Fixture Gain", vendor: "Fixture", license: "MIT", manifest,
      platforms: { [platform]: { library: process.platform === "darwin" ? "gain.dylib" : "gain.so", sha256 } } };
    await writeFile(join(pkg, "plugins.json"), JSON.stringify({ formatVersion: 1, plugins: [plugin,
      { ...plugin, displayName: "Other platform", manifest: { ...manifest, pluginId: "fixture.other" }, platforms: {} },
      { ...plugin, displayName: "Missing", manifest: { ...manifest, pluginId: "fixture.missing" }, platforms: { [platform]: { library: "missing.dylib", sha256 } } },
    ] }));
    const discovery = await discoverProjectPlugins(root);
    expect(discovery.plugins.map(plugin => plugin.entry.availability)).toEqual(["available", "unsupported", "missing"]);
    const catalog = new ProjectPluginCatalog(root);
    const frame: PreviewSnapshotFrame = { protocolVersion: "1.0", type: "snapshot", hash: "0".repeat(64), snapshot: new Project().snapshot(), plugins: [], assetBaseDir: root, allowPlugins: "any" };
    await catalog.refresh(frame, () => {});
    expect(catalog.view().filter(plugin => plugin.source === "builtin")).toHaveLength(20);
    const gain = catalog.view().find(plugin => plugin.pluginId === "fixture.gain")!;
    expect(gain).toMatchObject({ packageVersion: "1.2.3", pluginVersion: "1.0.0", validation: "unverified", usages: [] });
    await catalog.verify(gain.handle, frame, new AbortController().signal, () => {});
    expect(catalog.view().find(plugin => plugin.handle === gain.handle)).toMatchObject({ validation: "verified", sha256 });
    await writeFile(binary, "broken binary");
    await expect(catalog.checkReads()).rejects.toMatchObject({ code: "SourceChanged" });
    await catalog.refresh(frame, () => {});
    expect(catalog.view().find(plugin => plugin.handle === gain.handle)!.validation).toBe("unverified");
    await expect(catalog.verify(gain.handle, frame, new AbortController().signal, () => {})).rejects.toMatchObject({ code: "PluginManifestMismatch" });
    expect(catalog.view().find(plugin => plugin.handle === gain.handle)!.validation).toBe("failed");
    await writeFile(join(pkg, "package.json"), '{"name":"catalog-fixture","version":"1.2.3","oxitone":{"plugins":"../../package.json"}}');
    const invalid = await discoverProjectPlugins(root);
    expect(invalid.plugins[0]!.entry).toMatchObject({ availability: "invalid" });
    expect(invalid.plugins[0]!.entry.diagnostic).toContain("escapes its package");
  } finally { await rm(root, { recursive: true, force: true }); }
});

it("runs plugin install, repair and uninstall as validated no-shell package tasks", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-plugin-tasks-"));
  const calls: string[][] = [];
  try {
    await writeFile(join(root, "package.json"), '{"name":"fixture","dependencies":{}}');
    const runner: PluginTaskRunner = { async run(args) {
      calls.push([...args]);
      const packagePath = join(root, "package.json");
      const packageJson = JSON.parse(await readFile(packagePath, "utf8")) as { dependencies: Record<string, string> };
      packageJson.dependencies ??= {};
      if (args[0] === "add") packageJson.dependencies["@acme/synth"] = "2.1.0";
      if (args[0] === "remove") delete packageJson.dependencies[args[1]!];
      await writeFile(packagePath, JSON.stringify(packageJson));
    } };
    const lifecycle = new PluginLifecycle(root, runner);
    const signal = new AbortController().signal;
    await lifecycle.run({ kind: "install", packageName: "@acme/synth", version: "2.1.0" }, signal);
    await lifecycle.run({ kind: "upgrade", packageName: "@acme/synth", version: "2.2.0" }, signal);
    await lifecycle.run({ kind: "repair", packageName: "@acme/synth" }, signal);
    await lifecycle.run({ kind: "uninstall", packageName: "@acme/synth" }, signal);
    expect(calls).toEqual([["add", "--save-exact", "@acme/synth@2.1.0"], ["update", "--save-exact", "@acme/synth@2.2.0"], ["install", "--force", "--filter", "@acme/synth"], ["remove", "@acme/synth"]]);
    await expect(lifecycle.run({ kind: "install", packageName: "--evil" }, signal)).rejects.toMatchObject({ code: "PluginManifestMismatch" });
    await expect(lifecycle.run({ kind: "upgrade", packageName: "@acme/synth", version: "../../escape" }, signal)).rejects.toMatchObject({ code: "PluginManifestMismatch" });
    await expect(lifecycle.run({ kind: "install", packageName: "@acme/synth", version: "../../escape" }, signal)).rejects.toMatchObject({ code: "PluginManifestMismatch" });
  } finally { await rm(root, { recursive: true, force: true }); }
});

it("restores package manifests when a lifecycle task fails after partial mutation", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-plugin-rollback-"));
  try {
    const packageJson = '{"name":"fixture","dependencies":{"@acme/synth":"2.1.0"}}';
    const lock = "lock-before\n";
    await writeFile(join(root, "package.json"), packageJson);
    await writeFile(join(root, "pnpm-lock.yaml"), lock);
    const lifecycle = new PluginLifecycle(root, { async run(_args) {
      await writeFile(join(root, "package.json"), '{"name":"fixture","dependencies":{"@acme/synth":"9.9.9"}}');
      await writeFile(join(root, "pnpm-lock.yaml"), "lock-after\n");
      throw new Error("registry unavailable");
    } });
    await expect(lifecycle.run({ kind: "upgrade", packageName: "@acme/synth", version: "9.9.9" }, new AbortController().signal))
      .rejects.toMatchObject({ code: "PluginInstallFailed" });
    expect(await readFile(join(root, "package.json"), "utf8")).toBe(packageJson);
    expect(await readFile(join(root, "pnpm-lock.yaml"), "utf8")).toBe(lock);
  } finally { await rm(root, { recursive: true, force: true }); }
});
