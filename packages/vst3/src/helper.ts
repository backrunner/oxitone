import { spawn } from "node:child_process";
import { executable } from "./resolve-host.js";
export { executable } from "./resolve-host.js";
import { ErrorCode, ERROR_CODES, OxitoneError, type OxitoneErrorCode } from "@oxitone/protocol";
import type { Readable } from "node:stream";

export interface Vst3HostOptions {
  /** Explicit local executable. No downloads or runtime builds. */
  hostPath?: string;
  /** Whole operation, including loading and teardown. Default 30 s; maximum 10 min. */
  timeoutMs?: number;
  signal?: AbortSignal;
}
const MAX_BYTES = 8 * 1024 * 1024;

/** One native process per operation. stdout/stderr never carry protocol data or enter JS audio processing. */
export async function runHelper(request: unknown, options: Vst3HostOptions): Promise<unknown> {
  const timeout = options.timeoutMs ?? 30_000;
  if (!Number.isInteger(timeout) || timeout < 1 || timeout > 600_000)
    throw new OxitoneError(ErrorCode.InvalidProject, "timeoutMs must be an integer in 1..600000");
  options.signal?.throwIfAborted();
  const path = await executable(options);
  const input = JSON.stringify(request);
  if (Buffer.byteLength(input) > MAX_BYTES)
    throw new OxitoneError(ErrorCode.BudgetExceeded, "VST3 request exceeds 8 MiB");
  options.signal?.throwIfAborted();
  return new Promise((resolve, reject) => {
    const child = spawn(path, [], { detached: true, stdio: ["pipe", "ignore", "ignore", "pipe"] });
    const output = child.stdio[3] as Readable;
    const stdin = child.stdin;
    let failure: Error | undefined;
    let size = 0;
    const chunks: Buffer[] = [];
    const kill = (error: Error) => {
      failure ??= error;
      if (child.pid) {
        try {
          process.kill(-child.pid, "SIGKILL");
        } catch {
          child.kill("SIGKILL");
        }
      }
      output.destroy();
      stdin?.destroy();
    };
    const abort = () => kill(new DOMException("VST3 operation aborted", "AbortError"));
    const timer = setTimeout(
      () => kill(new OxitoneError(ErrorCode.PluginHostTimeout, "VST3 helper timed out")),
      timeout,
    );
    options.signal?.addEventListener("abort", abort, { once: true });
    if (options.signal?.aborted) abort();
    output.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > MAX_BYTES) kill(new OxitoneError(ErrorCode.BudgetExceeded, "VST3 response exceeds 8 MiB"));
      else chunks.push(chunk);
    });
    output.on("error", (error) => kill(error));
    stdin?.on("error", () => {
      /* close reports crashes/EPIPE without an unhandled stream error */
    });
    child.once("error", (cause) => {
      failure ??= new OxitoneError(ErrorCode.PluginHostUnavailable, "Cannot start VST3 helper", { cause });
    });
    child.once("close", (code, signal) => {
      clearTimeout(timer);
      options.signal?.removeEventListener("abort", abort);
      if (failure) return reject(failure);
      if (code !== 0 || signal)
        return reject(new OxitoneError(ErrorCode.PluginHostCrashed, `VST3 helper exited (${signal ?? code})`));
      try {
        const reply: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
        if (typeof reply !== "object" || reply === null || !("protocolVersion" in reply) || reply.protocolVersion !== 1)
          throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "Unsupported VST3 helper response");
        if ("error" in reply) {
          const error = reply.error as { code?: unknown; message?: unknown };
          if (
            !error ||
            typeof error.code !== "string" ||
            !ERROR_CODES.includes(error.code as OxitoneErrorCode) ||
            typeof error.message !== "string"
          )
            throw new OxitoneError(ErrorCode.RealtimeFault, "Malformed VST3 helper error");
          throw new OxitoneError(error.code as OxitoneErrorCode, error.message);
        }
        resolve(reply);
      } catch (cause) {
        reject(
          cause instanceof OxitoneError
            ? cause
            : new OxitoneError(ErrorCode.RealtimeFault, "Malformed VST3 helper response", { cause }),
        );
      }
    });
    stdin?.end(input);
  });
}
