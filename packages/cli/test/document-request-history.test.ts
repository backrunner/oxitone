import { expect, it } from "vitest";
import { DocumentDispatcher } from "../src/source/document/document-dispatch.js";
import type { ProjectDocument } from "../src/source/document/project-document.js";

it("keeps a long-lived client writable after thousands of requests without replaying expired commands", async () => {
  let saves = 0;
  const document = { sessionId: "session", view: { revision: 0 }, save: async () => { saves++; } } as unknown as ProjectDocument;
  const dispatcher = new DocumentDispatcher(document);
  const request = (sequence: number, kind = "query") => ({ documentProtocolVersion: "2.0", sessionId: "session", requestId: `stream/editor-a/${sequence}`, baseRevision: 0, operation: { kind } });
  expect(await dispatcher.submit(request(1, "save"))).toMatchObject({ accepted: true });
  for (let sequence = 2; sequence <= 5000; sequence++) expect(await dispatcher.submit(request(sequence))).toMatchObject({ accepted: true });
  expect(await dispatcher.submit(request(1, "save"))).toMatchObject({ accepted: false, error: { code: "SourceChanged" } });
  expect(saves).toBe(1);
  expect(await dispatcher.submit(request(5001, "save"))).toMatchObject({ accepted: true });
  expect(await dispatcher.submit(request(5001, "save"))).toMatchObject({ accepted: true });
  expect(saves).toBe(2);
  dispatcher.close();
});

it("bounds client streams and rejects unsupported or reused command identities", async () => {
  let saves = 0;
  const document = { sessionId: "session", view: { revision: 0 }, save: async () => { saves++; } } as unknown as ProjectDocument;
  const dispatcher = new DocumentDispatcher(document);
  const request = (requestId: string, kind = "query") => ({ documentProtocolVersion: "2.0", sessionId: "session", requestId, baseRevision: 0, operation: { kind } });
  expect(await dispatcher.submit(request("old-request", "save"))).toMatchObject({ accepted: false, error: { code: "SourceChanged" } });
  expect(await dispatcher.submit(request("stream/editor/1", "save"))).toMatchObject({ accepted: true });
  expect(await dispatcher.submit(request("stream/editor/1"))).toMatchObject({ accepted: false, error: { code: "SourceChanged" } });
  for (const id of ["stream/editor/01", "stream/editor/0", "stream/editor/9007199254740992"]) {
    expect(await dispatcher.submit(request(id))).toMatchObject({ accepted: false, error: { code: "SourceChanged" } });
  }
  for (let client = 1; client < 64; client++) expect(await dispatcher.submit(request(`stream/client-${client}/1`))).toMatchObject({ accepted: true });
  expect(await dispatcher.submit(request("stream/overflow/1"))).toMatchObject({ accepted: false, error: { code: "BudgetExceeded" } });
  expect(await dispatcher.submit(request("stream/editor/2", "save"))).toMatchObject({ accepted: true });
  expect(saves).toBe(2); dispatcher.close();
});
