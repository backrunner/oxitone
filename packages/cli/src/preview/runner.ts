import { spawn, type ChildProcess } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, statSync, watch, type FSWatcher } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { context, type BuildContext } from "esbuild";
import { canonicalEncode, PREVIEW_MAX_FRAME_BYTES, previewFrameSchema, type PreviewFrame } from "@oxitone/protocol";
import { bundleCode, projectBuildOptions, writeBundle } from "../bundle.js";

const require = createRequire(import.meta.url);
export interface RunnerOptions {
  watch?: boolean;
  debounceMs?: number;
  watchPaths?: string[];
  timeoutMs?: number;
}

/** Watches dependencies and executes each version in an isolated Node process. */
export class PreviewRunner {
  private buildContext?: BuildContext;
  private child: ChildProcess | undefined;
  private timer: NodeJS.Timeout | undefined;
  private watchers: FSWatcher[] = [];
  private generation = 0;
  private revision = 0n;
  private hash: string | undefined;
  private closed = false;
  private directory = "";
  private bundle = "";

  constructor(
    private readonly entry: string,
    private readonly send: (frame: PreviewFrame) => void,
    private readonly options: RunnerOptions = {},
  ) {}

  async start(): Promise<void> {
    this.directory = await mkdtemp(join(tmpdir(), "oxitone-project-"));
    const options = projectBuildOptions(this.entry);
    this.buildContext = await context({
      ...options,
      plugins: [
        ...options.plugins!,
        {
          name: "oxitone-preview",
          setup: (build) => {
            build.onStart(() => {
              this.generation++;
              this.child?.kill("SIGKILL");
              if (this.timer) clearTimeout(this.timer);
              this.send({ type: "status", protocolVersion: "1.0", state: "building" });
            });
            build.onEnd((result) => {
              if (this.closed) return;
              if (result.errors.length) {
                const error = result.errors[0]!;
                this.diagnostic(
                  error.text,
                  error.location ? `${error.location.file}:${error.location.line}` : undefined,
                );
              } else if (result.outputFiles?.length === 1) {
                this.bundle = bundleCode(result, this.entry);
                this.schedule();
              } else this.diagnostic("Project build must produce exactly one JavaScript file");
            });
          },
        },
      ],
    });
    if (this.options.watch === false) await this.buildContext.rebuild();
    else await this.buildContext.watch();
    for (const path of this.options.watchPaths ?? []) {
      const target = resolve(path);
      const directory = statSync(target).isDirectory();
      // Watch the parent of individual files so atomic editor replacements survive.
      const root = directory ? target : dirname(target);
      const watcher = watch(root, { recursive: true }, (_, filename) => {
        if (!directory && filename !== null && resolve(root, String(filename)) !== target) return;
        this.generation++;
        this.child?.kill("SIGKILL");
        this.send({ type: "status", protocolVersion: "1.0", state: "building" });
        void this.buildContext?.rebuild().catch((error) => this.diagnostic(String(error)));
      });
      watcher.on("error", (error) => this.diagnostic(error.message, target));
      this.watchers.push(watcher);
    }
  }

  private diagnostic(message: string, path?: string): void {
    this.send({
      type: "diagnostic",
      protocolVersion: "1.0",
      code: "PreviewBuildFailed",
      message,
      ...(path === undefined ? {} : { path }),
    });
  }

  private schedule(): void {
    if (this.timer) clearTimeout(this.timer);
    const generation = this.generation;
    this.timer = setTimeout(() => {
      void this.evaluate(generation).catch((error) => {
        if (!this.closed && generation === this.generation) this.diagnostic(String(error));
      });
    }, this.options.debounceMs ?? 150);
  }

  private async evaluate(generation: number): Promise<void> {
    if (this.closed || generation !== this.generation) return;
    const bundle = join(this.directory, `project-${generation}.mjs`);
    await writeBundle(bundle, this.bundle);
    if (this.closed || generation !== this.generation) {
      await rm(bundle, { force: true });
      return;
    }
    const worker = new URL("./evaluate.js", import.meta.url);
    if (!existsSync(worker)) worker.pathname = worker.pathname.replace(/\.js$/, ".ts");
    const child = spawn(
      process.execPath,
      ["--import", require.resolve("tsx"), fileURLToPath(worker), bundle, resolve(this.entry)],
      {
        cwd: dirname(resolve(this.entry)),
        stdio: ["ignore", "pipe", "pipe", "pipe"],
      },
    );
    this.child = child;
    const chunks: Buffer[] = [];
    let bytes = 0;
    let diagnostics = "";
    let loggedBytes = 0;
    const timer = setTimeout(() => {
      diagnostics = "Preview execution timed out";
      child.kill("SIGKILL");
    }, this.options.timeoutMs ?? 10_000);
    child.stdout?.on("data", (chunk: Buffer) => {
      if (loggedBytes < 16_384) process.stderr.write(chunk.subarray(0, 16_384 - loggedBytes));
      loggedBytes += chunk.length;
    });
    child.stderr?.on("data", (chunk: Buffer) => {
      diagnostics = (diagnostics + chunk.toString()).slice(-16_384);
    });
    child.stdio[3]?.on("data", (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > PREVIEW_MAX_FRAME_BYTES) {
        diagnostics = "Preview result exceeds 64 MiB";
        child.kill("SIGKILL");
      } else chunks.push(chunk);
    });
    child.on("error", (error) => {
      diagnostics = error.message;
    });
    child.on("close", (code) => {
      void rm(bundle, { force: true });
      clearTimeout(timer);
      if (this.closed || generation !== this.generation) return;
      this.child = undefined;
      if (code !== 0 || bytes > PREVIEW_MAX_FRAME_BYTES) {
        this.diagnostic(diagnostics || `Preview execution exited ${code}`);
        return;
      }
      try {
        const value = JSON.parse(Buffer.concat(chunks).toString("utf8"));
        value.snapshot.revision = "0";
        const hash = createHash("sha256").update(canonicalEncode(value)).digest("hex");
        if (hash === this.hash) {
          this.send({ type: "status", protocolVersion: "1.0", state: "watching" });
          return;
        }
        // Changed source remains Building until native acceptance (or a rejection diagnostic).
        if (this.revision === 0xffff_ffff_ffff_ffffn) throw new Error("Preview revision exhausted");
        value.snapshot.revision = String(++this.revision);
        const frame = previewFrameSchema.parse({ ...value, hash, type: "snapshot", protocolVersion: "1.0" });
        this.send(frame);
        this.hash = hash;
      } catch (error) {
        this.diagnostic(error instanceof Error ? error.message : String(error));
      }
    });
  }

  async close(): Promise<void> {
    this.closed = true;
    if (this.timer) clearTimeout(this.timer);
    this.child?.kill("SIGKILL");
    for (const watcher of this.watchers) watcher.close();
    await this.buildContext?.dispose();
    if (this.directory) await rm(this.directory, { recursive: true, force: true });
  }

  /** A failed native compile must be retried on a subsequent asset/watch event. */
  rejectRevision(revision: string): void {
    if (revision === String(this.revision)) this.hash = undefined;
  }
}
