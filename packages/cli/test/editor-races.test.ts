import { createServer, type Socket } from "node:net";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { expect, it } from "vitest";
import type { DocumentView, DocumentRequest } from "@oxitone/protocol";
import { EditorBuffers } from "../src/editor/buffers.js";
import { EditorDocumentSession } from "../src/editor/session.js";
import { FrameDecoder, encodeFrame } from "../src/preview/framing.js";
import { until } from "./preview-helpers.js";

const fileName = "/tmp/song.ts";
const view = (text: string, revision: number, sessionId = "session-a"): DocumentView => ({
  sessionId,
  projectRoot: "/tmp",
  revision,
  acceptedRevision: revision,
  savedRevision: 0,
  status: "ready",
  saving: false,
  modified: text !== "A",
  sites: [],
  automationSites: [],
  configurationSites: [],
  rackSites: [],
  plugins: [],
  conflicts: [],
  files: [{ path: fileName, text }],
});

it("finishes an already proposed remote edit before applying a newer DAW revision", () => {
  const buffers = new EditorBuffers();
  buffers.open({ fileName, text: "A", baselineText: "A", version: 1 });
  buffers.reconcile(view("A", 0));
  buffers.reconcile(view("B", 1));
  const pending = buffers.values.get(fileName)!.apply!;
  buffers.reconcile(view("C", 2));
  buffers.applied(fileName, pending.expectedVersion, 2, pending.text);
  buffers.reconcile(view("C", 2));
  expect(buffers.values.get(fileName)).toMatchObject({
    text: "B",
    baselineText: "B",
    apply: { text: "C", expectedVersion: 2 },
  });
  expect(buffers.values.get(fileName)!.conflict).toBeUndefined();
});

it("retains synchronized but unsaved editor text when a restarted service loads older disk contents", () => {
  const buffers = new EditorBuffers();
  buffers.open({ fileName, text: "A", baselineText: "A", version: 1 });
  buffers.reconcile(view("A", 0));
  buffers.change(fileName, "B", 2);
  buffers.reconcile(view("B", 1), { fileName, text: "B" });
  buffers.reconcile(view("A", 0, "session-b"));
  const result = buffers.values.get(fileName)!;
  expect(result.text).toBe("B");
  expect(result.apply).toBeUndefined();
  expect(result.conflict).toMatchObject({ local: "B", remote: "A", reason: "sessionChanged" });
});

it("honors a correlated code acknowledgement even when a later DAW event arrives in the same socket batch", async () => {
  const root = await mkdtemp("/tmp/oxitone-editor-race-");
  const clients = new Set<Socket>();
  const event = (text: string, revision: number) =>
    encodeFrame({ documentProtocolVersion: "2.0", type: "event", view: view(text, revision) });
  const server = createServer((socket) => {
    clients.add(socket);
    socket.on("close", () => clients.delete(socket));
    const decoder = new FrameDecoder();
    socket.write(event("A", 0));
    socket.on("data", (data) => {
      for (const raw of decoder.push(data as Buffer)) {
        const request = raw as DocumentRequest;
        socket.write(
          Buffer.concat([
            event("B", 1),
            encodeFrame({
              documentProtocolVersion: "2.0",
              type: "response",
              sessionId: "session-a",
              requestId: request.requestId,
              accepted: true,
              revision: 1,
            }),
            event("C", 2),
          ]),
        );
      }
    });
  });
  const socket = join(root, "socket");
  await new Promise<void>((accept) => server.listen(socket, accept));
  const editor = new EditorDocumentSession(socket);
  try {
    editor.openBuffer({ fileName, text: "A", baselineText: "A", version: 1 });
    await until(() => editor.view.state === "connected");
    editor.changeBuffer(fileName, "B", 2);
    await until(() => editor.view.document?.revision === 2 && !editor.view.submitting);
    expect(editor.view.buffers[0]!.conflict).toBeUndefined();
    expect(editor.view.buffers[0]).toMatchObject({ baselineText: "B", apply: { text: "C" } });
  } finally {
    editor.close();
    for (const client of clients) client.destroy();
    await new Promise<void>((accept) => server.close(() => accept()));
    await rm(root, { recursive: true, force: true });
  }
});

it("serializes semantic requests and retains typing until the in-flight command completes", async () => {
  const root = await mkdtemp("/tmp/oxitone-editor-command-");
  const clients = new Set<Socket>(),
    requests: DocumentRequest[] = [];
  let release: (() => void) | undefined;
  const server = createServer((socket) => {
    clients.add(socket);
    socket.on("close", () => clients.delete(socket));
    const decoder = new FrameDecoder();
    const event = (text: string, revision: number) =>
      encodeFrame({ documentProtocolVersion: "2.0", type: "event", view: view(text, revision) });
    socket.write(event("A", 0));
    socket.on("data", (data) => {
      for (const raw of decoder.push(data as Buffer)) {
        const request = raw as DocumentRequest;
        requests.push(request);
        const answer = () =>
          socket.write(
            Buffer.concat([
              event(request.operation.kind === "code" ? request.operation.text : "A", requests.length),
              encodeFrame({
                documentProtocolVersion: "2.0",
                type: "response",
                sessionId: "session-a",
                requestId: request.requestId,
                accepted: true,
                revision: requests.length,
              }),
            ]),
          );
        if (request.operation.kind === "createFile") release = answer;
        else answer();
      }
    });
  });
  const socket = join(root, "socket");
  await new Promise<void>((accept) => server.listen(socket, accept));
  const editor = new EditorDocumentSession(socket);
  try {
    editor.openBuffer({ fileName, text: "A", baselineText: "A", version: 1 });
    await until(() => editor.view.state === "connected");
    const pending = editor.request({ kind: "createFile", fileName: "/tmp/new.ts", text: "" });
    await until(() => !!release);
    editor.changeBuffer(fileName, "B", 2);
    await expect(editor.command("save")).rejects.toMatchObject({ code: "SourceChanged" });
    await expect(editor.request({ kind: "installPlugin", packageName: "synth" })).rejects.toMatchObject({
      code: "SourceChanged",
    });
    expect(requests.map((request) => request.operation.kind)).toEqual(["createFile"]);
    release!();
    expect((await pending).accepted).toBe(true);
    await editor.flush();
    expect(requests.map((request) => request.operation.kind)).toEqual(["createFile", "code"]);
    expect(requests[1]!.baseRevision).toBe(1);
    expect(editor.view.buffers[0]).toMatchObject({ text: "B", baselineText: "B" });
  } finally {
    editor.close();
    for (const client of clients) client.destroy();
    await new Promise<void>((accept) => server.close(() => accept()));
    await rm(root, { recursive: true, force: true });
  }
});
