import { createServer, type Server } from "node:net";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { expect, it } from "vitest";
import { PreviewVst3Client } from "../src/preview/vst3-client.js";
import { encodeFrame, FrameDecoder } from "../src/preview/framing.js";

async function fixture(
  run: (client: PreviewVst3Client, requests: Record<string, unknown>[]) => Promise<void>,
  respond: (request: Record<string, unknown>) => unknown,
) {
  const root = await mkdtemp("/tmp/oxi-vst3-client-");
  const path = join(root, "ipc");
  const requests: Record<string, unknown>[] = [];
  let server: Server | undefined;
  try {
    server = createServer((socket) => {
      const decoder = new FrameDecoder();
      socket.on("error", () => {});
      socket.on("data", (chunk: Buffer) => {
        for (const request of decoder.push(chunk) as Record<string, unknown>[]) {
          requests.push(request);
          const response = respond(request);
          if (response) {
            const bytes = encodeFrame(response);
            socket.write(bytes.subarray(0, 3));
            setImmediate(() => socket.end(bytes.subarray(3)));
          }
        }
      });
    });
    await new Promise<void>((resolve) => server!.listen(path, resolve));
    await run(new PreviewVst3Client(path), requests);
  } finally {
    if (server) await new Promise<void>((resolve) => server!.close(() => resolve()));
    await rm(root, { recursive: true, force: true });
  }
}
const inventory = { instanceControlVersion: 1, graphGeneration: "10", state: "active", instances: [] };
it("correlates snapshot and instance identity over private, independent requests", async () => {
  await fixture(
    async (client, requests) => {
      expect(await client.inventory("2", new AbortController().signal)).toEqual(inventory);
      const result = await client.control(
        "2",
        { instanceControlVersion: 1, graphGeneration: "10", instanceId: "ins_gain", command: { kind: "poll" } },
        new AbortController().signal,
      );
      expect(result.state.nextSequence).toBe(7);
      expect(requests.map((r) => r.type)).toEqual(["vst3Instances", "vst3Control"]);
    },
    (request) =>
      request.type === "vst3Instances"
        ? { protocolVersion: "1.0", type: "vst3Instances", snapshotRevision: "2", inventory }
        : {
            protocolVersion: "1.0",
            type: "vst3Control",
            snapshotRevision: "2",
            result: {
              instanceControlVersion: 1,
              graphGeneration: "10",
              instanceId: "ins_gain",
              state: { editorOpen: false, nextSequence: 7 },
            },
          },
  );
});
it("rejects stale revisions and preserves native errors", async () => {
  for (const response of [
    { protocolVersion: "1.0", type: "vst3Instances", snapshotRevision: "1", inventory },
    { protocolVersion: "1.0", type: "rejected", code: "PluginTaskConflict", message: "old graph" },
  ])
    await fixture(
      async (client) => {
        await expect(client.inventory("2", new AbortController().signal)).rejects.toMatchObject({
          code: response.type === "rejected" ? "PluginTaskConflict" : "SourceChanged",
        });
      },
      () => response,
    );
});
it("cancels waiting RPCs without poisoning another connection", async () => {
  await fixture(
    async (client) => {
      const controller = new AbortController();
      const pending = client.inventory("2", controller.signal);
      controller.abort();
      await expect(pending).rejects.toMatchObject({ code: "SourceChanged" });
      await expect(client.inventory("3", new AbortController().signal)).resolves.toEqual(inventory);
    },
    (request) =>
      request.snapshotRevision === "3"
        ? { protocolVersion: "1.0", type: "vst3Instances", snapshotRevision: "3", inventory }
        : undefined,
  );
});
