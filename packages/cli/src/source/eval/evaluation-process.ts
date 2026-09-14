import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { readEvaluationFailure } from "./evaluation-failure.js";
import { sourceSpan } from "./source-timing.js";

const require = createRequire(import.meta.url);
const MAX_RESULT = 64 * 1024 * 1024;

/** The child is disposable, bounded, and never part of an audio callback. It is not a JS security sandbox. */
export async function runEvaluationProcess(bundle: string, key: string, cwd: string, signal: AbortSignal, timeoutMs: number, workerName: "project-worker" | "plugin-worker"): Promise<unknown> {
  signal.throwIfAborted();
  const worker = new URL(`./${workerName}.js`, import.meta.url);
  if (!existsSync(worker)) worker.pathname = worker.pathname.replace(/\.js$/, ".ts");
  // Built host modules are JavaScript. The project worker installs TS support just before user imports.
  const preload = worker.pathname.endsWith(".ts") ? ["--import", require.resolve("tsx")] : [];
  const done = sourceSpan("worker");
  return new Promise((accept, reject) => {
    const child = spawn(process.execPath, [...preload, fileURLToPath(worker), bundle, key],
      { cwd, stdio: ["ignore", "ignore", "pipe", "pipe"] });
    const chunks: Buffer[] = [];
    let bytes = 0;
    let diagnostics = "";
    let failure: Error | undefined;
    const stop = (error: Error) => { failure ??= error; child.kill("SIGKILL"); };
    const abort = () => stop(new OxitoneError(ErrorCode.SourceChanged, "source evaluation cancelled"));
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
    const timer = setTimeout(() => stop(new OxitoneError(ErrorCode.BudgetExceeded, "source execution timed out")), timeoutMs);
    child.stderr?.on("data", (chunk: Buffer) => { diagnostics = (diagnostics + chunk.toString()).slice(-16_384); });
    child.stdio[3]?.on("data", (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > MAX_RESULT) stop(new OxitoneError(ErrorCode.BudgetExceeded, "source evaluation result exceeds 64 MiB"));
      else chunks.push(chunk);
    });
    child.on("error", error => { failure ??= error; });
    child.on("close", code => {
      clearTimeout(timer); signal.removeEventListener("abort", abort);
      if (failure) { reject(failure); return; }
      if (code !== 0) { reject(readEvaluationFailure(Buffer.concat(chunks)) ?? new OxitoneError(ErrorCode.DraftInvalid, diagnostics || `source execution exited ${code}`)); return; }
      try { accept(JSON.parse(Buffer.concat(chunks).toString("utf8"))); } catch (error) { reject(error); }
    });
  }).finally(done);
}
