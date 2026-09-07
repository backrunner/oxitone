import { spawn, type ChildProcess } from "node:child_process";
import { access, chmod, mkdtemp, rm } from "node:fs/promises";
import { constants } from "node:fs";
import { createRequire } from "node:module";
import { connect, type Socket } from "node:net";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { previewResponseSchema, type PreviewFrame } from "@oxitone/protocol";
import { FrameDecoder, encodeFrame } from "./framing.js";
import { PreviewRunner, type RunnerOptions } from "./runner.js";

export interface PreviewOptions extends RunnerOptions { viewer?: string; headless?: boolean; }

export function parsePreviewArgs(args: string[]): { entry: string; options: PreviewOptions } {
  const entry = args.shift();
  if (!entry || entry.startsWith("--")) throw new Error("Usage: oxitone preview <entry.ts> [--no-watch] [--watch-path path] [--viewer path] [--headless]");
  const options: PreviewOptions = { watch: true, watchPaths: [] };
  for (let index = 0; index < args.length; index++) {
    const arg = args[index];
    if (arg === "--watch") options.watch = true;
    else if (arg === "--no-watch") options.watch = false;
    else if (arg === "--headless") options.headless = true;
    else if (arg === "--viewer" || arg === "--watch-path") {
      const value = args[++index];
      if (!value || value.startsWith("--")) throw new Error(`${arg} requires a path`);
      if (arg === "--viewer") options.viewer = resolve(value);
      else options.watchPaths!.push(resolve(value));
    } else throw new Error(`Unknown preview option: ${arg}`);
  }
  return { entry: resolve(entry), options };
}

async function viewerPath(explicit?: string, headless = false): Promise<string> {
  if (explicit) { await access(explicit, constants.X_OK); return explicit; }
  const require = createRequire(import.meta.url);
  const candidates: string[] = [];
  try { candidates.push(require.resolve(`@oxitone/preview-${process.platform}-${process.arch}/bin/oxitone-preview`)); } catch { /* Local development fallback. */ }
  for (const profile of ["release", "debug"]) {
    if (!headless) candidates.push(fileURLToPath(new URL(`../../../../target/${profile}/Oxitone Preview.app`, import.meta.url)));
    candidates.push(fileURLToPath(new URL(`../../../../target/${profile}/oxitone-preview`, import.meta.url)));
  }
  for (const path of candidates) { try { await access(path, constants.X_OK); return path; } catch { /* Try next installed binary. */ } }
  throw new Error("Preview binary unavailable. Run cargo build --release -p oxitone-preview, or pass --viewer <path>.");
}

async function openSocket(path: string, child: ChildProcess): Promise<Socket> {
  for (let attempt = 0; attempt < 200; attempt++) {
    if (child.exitCode !== null || child.signalCode !== null) throw new Error("Preview viewer exited before connecting");
    try {
      return await new Promise<Socket>((accept, reject) => {
        const socket = connect(path);
        socket.once("error", (error) => { socket.destroy(); reject(error); });
        socket.once("connect", () => { socket.removeAllListeners("error"); accept(socket); });
      });
    } catch { await delay(50); }
  }
  throw new Error("Preview viewer connection timed out");
}

/** One frame in flight; latest queued snapshots replace superseded work. */
export class PreviewConnection {
  private queue: PreviewFrame[] = [];
  private busy = false;
  private closed = false;
  private decoder = new FrameDecoder();
  private inFlight: PreviewFrame | undefined;
  constructor(private readonly socket: Socket, private readonly rejected: (revision: string) => void = () => {}) {
    socket.on("data", (chunk: Buffer) => {
      try {
        for (const value of this.decoder.push(chunk)) {
          const response = previewResponseSchema.parse(value);
          if (response.type === "rejected") {
            console.error(`[${response.code}] ${response.message}`);
            if (this.inFlight?.type === "snapshot") this.rejected(this.inFlight.snapshot.revision);
          }
          this.inFlight = undefined;
          this.busy = false;
          this.flush();
        }
      } catch (error) { socket.destroy(error as Error); }
    });
  }
  send(frame: PreviewFrame): void {
    if (this.closed) return;
    // Status, diagnostics and snapshots are replaceable presentation messages.
    this.queue = this.queue.filter((queued) => queued.type !== frame.type);
    this.queue.push(frame);
    this.flush();
  }
  private flush(): void {
    if (this.closed || this.busy || this.socket.destroyed) return;
    const frame = this.queue.shift();
    if (frame) { this.busy = true; this.inFlight = frame; this.socket.write(encodeFrame(frame)); }
  }
  close(): void { this.closed = true; this.queue = []; }
}

export async function launchPreview(entry: string, options: PreviewOptions = {}): Promise<void> {
  const binary = await viewerPath(options.viewer, options.headless);
  // Darwin's sockaddr_un is limited to 104 bytes; TMPDIR may already exceed it.
  const directory = await mkdtemp("/tmp/oxitone-preview-");
  await chmod(directory, 0o700);
  const path = join(directory, "ipc");
  const args = ["--socket", path, ...(options.headless ? ["--headless"] : [])];
  const bundle = binary.endsWith(".app");
  // Spawn the bundle's executable directly to retain the actual viewer PID.
  // `open -W` owns a separate process and cannot guarantee paired shutdown.
  const viewer = spawn(bundle ? join(binary, "Contents", "MacOS", "oxitone-preview") : binary, args, { stdio: "inherit" });
  let spawnError: Error | undefined;
  viewer.on("error", (error) => { spawnError = error; });
  let socket: Socket | undefined;
  let runner: PreviewRunner | undefined;
  let connection: PreviewConnection | undefined;
  let stop: (() => void) | undefined;
  const signal = () => stop?.();
  try {
    socket = await openSocket(path, viewer);
    if (spawnError) throw spawnError;
    connection = new PreviewConnection(socket, (revision) => runner?.rejectRevision(revision));
    let failure: Error | undefined;
    const finished = new Promise<void>((done) => {
      stop = done;
      viewer.once("exit", (code) => { if (code) failure = new Error(`Preview viewer exited ${code}`); done(); });
      socket!.once("error", (error) => { failure = error; done(); });
      socket!.once("close", done);
    });
    process.once("SIGINT", signal); process.once("SIGTERM", signal);
    runner = new PreviewRunner(entry, (frame) => {
      if (frame.type === "diagnostic") console.error(`[${frame.code}] ${frame.message}`);
      connection!.send(frame);
    }, options);
    await runner.start();
    console.error(`Oxitone preview · ${options.watch === false ? "single build" : "watching"} ${entry}`);
    await finished;
    if (failure) throw failure;
  } finally {
    process.removeListener("SIGINT", signal); process.removeListener("SIGTERM", signal);
    await runner?.close();
    connection?.close();
    if (socket && !socket.destroyed) socket.end(encodeFrame({ protocolVersion: "1.0", type: "shutdown" }));
    if (viewer.exitCode === null && viewer.signalCode === null) {
      await Promise.race([new Promise<void>((done) => viewer.once("exit", () => done())), delay(1500)]);
      if (viewer.exitCode === null && viewer.signalCode === null) viewer.kill("SIGKILL");
    }
    socket?.destroy();
    await rm(directory, { recursive: true, force: true });
  }
}
