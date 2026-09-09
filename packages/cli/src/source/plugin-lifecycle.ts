import { execFile } from "node:child_process";
import { access, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { promisify } from "node:util";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { npmPackageName } from "./plugin-discovery.js";

const runFile = promisify(execFile);
const packageSpec = /^(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*(?:@[a-zA-Z0-9][a-zA-Z0-9._+~-]*)?$/;

export type PluginTask =
  | { kind: "install"; packageName: string; version?: string }
  | { kind: "upgrade"; packageName: string; version?: string }
  | { kind: "uninstall"; packageName: string }
  | { kind: "repair"; packageName: string };

export interface PluginTaskRunner {
  run(args: readonly string[], cwd: string, signal: AbortSignal): Promise<void>;
}

const defaultRunner: PluginTaskRunner = {
  async run(args, cwd, signal) {
    try {
      await runFile(process.env.OXITONE_PACKAGE_MANAGER ?? "pnpm", [...args], {
        cwd, signal, timeout: 120_000, maxBuffer: 2 * 1024 * 1024, windowsHide: true,
      });
    } catch (error) {
      throw new OxitoneError(ErrorCode.PluginInstallFailed, error instanceof Error ? error.message : String(error), { cause: error });
    }
  },
};

/** Package lifecycle is an explicit control task; source materialization never calls it. */
export class PluginLifecycle {
  constructor(private readonly root: string, private readonly runner: PluginTaskRunner = defaultRunner) {}

  async run(task: PluginTask, signal: AbortSignal): Promise<void> {
    const packageName = task.packageName;
    if (!npmPackageName.test(packageName)) throw new OxitoneError(ErrorCode.PluginManifestMismatch, "invalid npm package name");
    const snapshot = await captureManifests(this.root);
    try {
      if (task.kind === "install" || task.kind === "upgrade") {
        const spec = task.version === undefined ? packageName : `${packageName}@${task.version}`;
        if (!packageSpec.test(spec) || spec.startsWith("-")) throw new OxitoneError(ErrorCode.PluginManifestMismatch, "invalid npm package version specifier");
        await this.runner.run([task.kind === "install" ? "add" : "update", "--save-exact", spec], this.root, signal);
      } else if (task.kind === "uninstall") {
        await this.runner.run(["remove", packageName], this.root, signal);
      } else {
        // `--force` repairs a missing or corrupt lock/node_modules entry without changing
        // package.json declarations. The package name is still checked to avoid a global repair.
        await this.runner.run(["install", "--force", "--filter", packageName], this.root, signal);
      }
    } catch (error) {
      await restoreManifests(this.root, snapshot);
      throw error instanceof OxitoneError ? error : new OxitoneError(ErrorCode.PluginInstallFailed, String(error), { cause: error });
    }
    try { await access(`${this.root}/package.json`); await readFile(`${this.root}/package.json`, "utf8"); }
    catch (error) {
      await restoreManifests(this.root, snapshot);
      throw new OxitoneError(ErrorCode.PluginInstallFailed, "package manager did not leave a readable project package.json", { cause: error });
    }
  }
}

type ManifestSnapshot = { path: string; text?: string }[];
const manifestNames = ["package.json", "pnpm-lock.yaml", "package-lock.json", "yarn.lock"] as const;
async function captureManifests(root: string): Promise<ManifestSnapshot> {
  const result: ManifestSnapshot = [];
  for (const name of manifestNames) {
    const path = join(root, name);
    try { result.push({ path, text: await readFile(path, "utf8") }); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") result.push({ path }); else throw error; }
  }
  return result;
}
async function restoreManifests(_root: string, snapshot: ManifestSnapshot): Promise<void> {
  for (const file of snapshot) {
    try {
      if (file.text === undefined) await rm(file.path, { force: true });
      else await writeFile(file.path, file.text, "utf8");
    } catch { /* Preserve the original package-manager error; diagnostics show recovery failure through the next refresh. */ }
  }
}
