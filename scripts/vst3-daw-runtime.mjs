// Production DawRunner/PreviewConnection with a real headless native backend and no device.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { connect } from "node:net";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { DawRunner } from "../packages/cli/dist/preview/daw-runner.js";
import { PreviewConnection } from "../packages/cli/dist/preview/launch.js";
import { PreviewVst3Client } from "../packages/cli/dist/preview/vst3-client.js";
import { FrameDecoder } from "../packages/cli/dist/preview/framing.js";

export async function startLiveDaw(entry, root) {
  const path = join(root, "preview.sock");
  const child = spawn(
    resolve(process.env.OXITONE_VST3_VIEWER ?? "target/debug/oxitone-preview"),
    ["--socket", path, "--headless"],
    {
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, OXITONE_PREVIEW_SIMULATED: "1" },
    },
  );
  let failure,
    output = "",
    socket,
    runner,
    connection,
    frame,
    view,
    state,
    sequence = 0;
  const responses = new Map();
  child.on("error", (error) => {
    failure = error;
  });
  for (const stream of [child.stdout, child.stderr])
    stream.on("data", (chunk) => {
      output = (output + chunk).slice(-8192);
    });
  async function until(check) {
    const end = performance.now() + 30_000;
    for (;;) {
      if (failure) throw failure;
      if (child.exitCode !== null || child.signalCode !== null) throw new Error(`Preview exited: ${output}`);
      if (await check()) return;
      if (performance.now() > end) throw new Error(`Live DAW timed out: ${output}`);
      await delay(20);
    }
  }
  async function close() {
    await runner?.close();
    connection?.send({ protocolVersion: "1.0", type: "shutdown" });
    for (let i = 0; i < 75 && child.exitCode === null && child.signalCode === null; i++) await delay(20);
    if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    connection?.close();
    socket?.destroy();
  }
  try {
    await until(async () => {
      try {
        socket = await new Promise((accept, reject) => {
          const candidate = connect(path);
          candidate.once("error", (error) => {
            candidate.destroy();
            reject(error);
          });
          candidate.once("connect", () => {
            candidate.removeAllListeners("error");
            accept(candidate);
          });
        });
        return true;
      } catch {
        return false;
      }
    });
    socket.on("error", (error) => {
      failure = error;
    });
    const decoder = new FrameDecoder();
    socket.on("data", (chunk) => {
      for (const response of decoder.push(chunk)) {
        if (response.type === "rejected") failure = new Error(JSON.stringify(response));
        else if (response.type === "state") state = response;
      }
    });
    connection = new PreviewConnection(
      socket,
      () => runner.rejectRevision(),
      (requests) => runner.receive(requests),
      true,
    );
    const runtime = new PreviewVst3Client(path);
    runner = new DawRunner(
      entry,
      (message) => {
        if (message.type === "snapshot") frame = message;
        if (message.type === "document") {
          if (message.message.type === "event") view = message.message.view;
          else responses.set(message.message.requestId, message.message);
        }
        connection.send(message);
      },
      { watch: false, vst3Runtime: runtime },
    );
    await runner.start();
    assert.equal(view.status, "ready", view.diagnostic?.message);
    const sync = async () => until(() => state?.revision === frame.snapshot.revision);
    await sync();
    return {
      get frame() {
        return frame;
      },
      get view() {
        return view;
      },
      get state() {
        return state;
      },
      runtime,
      until,
      sync,
      close,
      transport(command) {
        connection.send({ protocolVersion: "1.0", type: "transport", command });
      },
      async request(operation) {
        const requestId = `stream/live-smoke/${++sequence}`;
        runner.receive([
          {
            documentProtocolVersion: "2.0",
            sessionId: view.sessionId,
            requestId,
            baseRevision: view.revision,
            operation,
          },
        ]);
        await until(() => responses.has(requestId));
        const response = responses.get(requestId);
        responses.delete(requestId);
        assert.equal(response.accepted, true, JSON.stringify(response));
        await sync();
        return response;
      },
    };
  } catch (error) {
    await close();
    throw error;
  }
}
