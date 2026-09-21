import { expect, it, vi } from "vitest";
import { DocumentDispatcher } from "../src/source/document/document-dispatch.js";
import type { ProjectDocument } from "../src/source/document/project-document.js";

it.each([false, true])(
  "delivers cancel ahead of blocked native work while keeping ordinary requests ordered and idempotent (recording=%s)",
  async (recording) => {
    let finish!: () => void;
    let started!: () => void;
    const ready = new Promise<void>((resolve) => {
      started = resolve;
    });
    const pending = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const calls: string[] = [];
    const command = vi.fn(async (_revision, command) => {
      calls.push(command.kind);
      if (command.kind === (recording ? "stopRecording" : "render")) {
        started();
        await pending;
      }
      if (command.kind === (recording ? "cancelRecording" : "cancel")) finish();
    });
    const document = {
      sessionId: "session",
      view: { revision: 0 },
      vst3Command: command,
    } as unknown as ProjectDocument;
    const dispatcher = new DocumentDispatcher(document);
    const request = (sequence: number, command: object) => ({
      documentProtocolVersion: "2.0",
      sessionId: "session",
      baseRevision: 0,
      requestId: `stream/test/${sequence}`,
      operation: { kind: "vst3", command },
    });
    const rendering = dispatcher.submit(
      request(
        1,
        recording
          ? { kind: "stopRecording", recordingId: "test" }
          : { kind: "render", plugin: "x", options: { path: "/tmp/out.wav", frames: 1 } },
      ),
    );
    await ready;
    const cancel = request(2, recording ? { kind: "cancelRecording", recordingId: "test" } : { kind: "cancel" });
    const response = await dispatcher.submit(cancel);
    expect(response.accepted).toBe(true);
    expect((await rendering).accepted).toBe(true);
    expect(await dispatcher.submit(cancel)).toEqual(response);
    expect(calls).toEqual(recording ? ["stopRecording", "cancelRecording"] : ["render", "cancel"]);
    dispatcher.close();
  },
);

it("bounds completed cancels without evicting active work and admits cancel when the ordered queue is full", async () => {
  let finish!: () => void;
  let started!: () => void;
  const ready = new Promise<void>((resolve) => {
    started = resolve;
  });
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  let renders = 0;
  const document = {
    sessionId: "session",
    view: { revision: 0 },
    vst3Command: async (_revision: number, command: { kind: string }) => {
      if (command.kind === "render") {
        renders++;
        started();
        await pending;
      }
    },
  } as unknown as ProjectDocument;
  const dispatcher = new DocumentDispatcher(document);
  const request = (sequence: number, operation: object) => ({
    documentProtocolVersion: "2.0",
    sessionId: "session",
    baseRevision: 0,
    requestId: `stream/pressure/${sequence}`,
    operation,
  });
  const render = request(1, {
    kind: "vst3",
    command: { kind: "render", plugin: "x", options: { path: "/tmp/out.wav", frames: 1 } },
  });
  const rendering = dispatcher.submit(render);
  await ready;
  const cancel = { kind: "vst3", command: { kind: "cancel" } };
  for (let i = 2; i <= 302; i++) expect((await dispatcher.submit(request(i, cancel))).accepted).toBe(true);
  expect(dispatcher.submit(render)).toBe(rendering);
  expect(await dispatcher.submit(request(2, cancel))).toMatchObject({
    accepted: false,
    error: { code: "SourceChanged" },
  });
  const queued = Array.from({ length: 63 }, (_, i) => dispatcher.submit(request(303 + i, { kind: "query" })));
  expect(await dispatcher.submit(request(366, { kind: "query" }))).toMatchObject({
    accepted: false,
    error: { code: "BudgetExceeded" },
  });
  expect((await dispatcher.submit(request(367, cancel))).accepted).toBe(true);
  finish();
  await rendering;
  expect((await Promise.all(queued)).every((result) => result.accepted)).toBe(true);
  expect(renders).toBe(1);
  dispatcher.close();
});
