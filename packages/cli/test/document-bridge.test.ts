import { connect, type Socket } from "node:net";
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { documentMessageSchema, type DocumentMessage, type DocumentView } from "@oxitone/protocol";
import { ProjectDocument } from "../src/source/document/project-document.js";
import { DocumentDispatcher } from "../src/source/document/document-dispatch.js";
import { openDocumentBridge } from "../src/source/document/document-bridge.js";
import { encodeFrame, FrameDecoder } from "../src/preview/framing.js";
import { until } from "./preview-helpers.js";

it("synchronizes unsaved editor buffers over the real socket and deduplicates a graphical transaction independently of events", async () => {
  const root = await mkdtemp("/tmp/oxitone-document-bridge-");
  let document: ProjectDocument | undefined, close: (() => Promise<void>) | undefined, socket: Socket | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
    const entry = join(root, "song.ts");
    const source = "import { Project, chord } from '@oxitone/core'; const phrase = chord(60, 'major'); const project = new Project(); project.addTrack('Lead').add(phrase).at({ bar: 1 }); export default project;";
    await writeFile(entry, source);
    document = await ProjectDocument.open({ entry });
    const dispatcher = new DocumentDispatcher(document);
    close = await openDocumentBridge(join(root, "editor"), document, dispatcher);
    const messages: DocumentMessage[] = [], decoder = new FrameDecoder();
    socket = connect(join(root, "editor"));
    socket.on("data", (chunk: Buffer) => { messages.push(...decoder.push(chunk).map(value => documentMessageSchema.parse(value))); });
    await until(() => messages.length > 0);
    const initial = (messages[0] as Extract<DocumentMessage, { type: "event" }>).view;
    const request = { documentProtocolVersion: "2.0", sessionId: initial.sessionId, requestId: "stream/editor/1", baseRevision: 0,
      operation: { kind: "code", fileName: entry, text: source.replace("chord(60", "chord(62") } };
    socket.write(encodeFrame(request));
    await until(() => messages.some(message => message.type === "response" && message.requestId === request.requestId));
    expect(document.frame?.snapshot.patterns[0]?.notes[0]?.pitch).toBe(62);
    expect(await readFile(entry, "utf8")).toBe(source);
    const views = messages.filter((message): message is Extract<DocumentMessage, { type: "event" }> => message.type === "event");
    expect(views.some(message => message.view.status === "building")).toBe(true);
    const current: DocumentView = document.view;
    const site = current.sites.find(site => site.label === "phrase")!;
    const edit = { ...request, requestId: "stream/gpui/1", baseRevision: current.revision,
      operation: { kind: "notes", site: site.handle, edits: [{ select: { degree: 2 }, set: { pitch: 67 } }] } };
    const first = dispatcher.submit(edit), duplicate = dispatcher.submit(edit);
    expect(await duplicate).toEqual(await first); expect(document.view.revision).toBe(2);
    const reused = await dispatcher.submit({ ...edit, operation: { kind: "undo" } });
    expect(reused).toMatchObject({ accepted: false, error: { code: "SourceChanged" } });
    const save = await dispatcher.submit({ ...request, requestId: "stream/editor/2", baseRevision: 2, operation: { kind: "save" } });
    expect(save.accepted).toBe(true); expect(await readFile(entry, "utf8")).toContain(".edit(");
    const expired = await dispatcher.submit({ ...edit, requestId: "stream/gpui/2", baseRevision: 2 });
    expect(expired).toMatchObject({ accepted: false, error: { code: "EditTargetMissing" } });
    dispatcher.close();
  } finally { socket?.destroy(); await close?.(); document?.close(); await rm(root, { recursive: true, force: true }); }
});
