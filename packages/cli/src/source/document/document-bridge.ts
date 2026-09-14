import { chmod } from "node:fs/promises";
import { createServer, type Socket } from "node:net";
import { FrameDecoder, encodeFrame } from "../../preview/framing.js";
import type { ProjectDocument } from "./project-document.js";
import type { DocumentDispatcher } from "./document-dispatch.js";

/** Local editor bridge. Every connection receives the current session before sending versioned requests. */
export async function openDocumentBridge(path: string, document: ProjectDocument, dispatcher: DocumentDispatcher): Promise<() => Promise<void>> {
  const clients = new Set<Socket>();
  const server = createServer(socket => {
    clients.add(socket);
    const decoder = new FrameDecoder();
    const send = (message: unknown) => {
      if (socket.destroyed) return;
      if (socket.writableLength > 64 * 1024 * 1024) { socket.destroy(new Error("editor bridge backpressure limit exceeded")); return; }
      socket.write(encodeFrame(message));
    };
    const unsubscribe = document.subscribe(view => send({ documentProtocolVersion: "2.0", type: "event", view }));
    socket.on("data", (chunk: Buffer) => {
      try { for (const request of decoder.push(chunk)) void dispatcher.submit(request).then(send).catch(error => socket.destroy(error)); }
      catch (error) { socket.destroy(error as Error); }
    });
    socket.on("error", () => { /* A failed editor does not terminate the shared document. */ });
    socket.on("close", () => { unsubscribe(); clients.delete(socket); });
  });
  await new Promise<void>((accept, reject) => { server.once("error", reject); server.listen(path, () => { server.off("error", reject); accept(); }); });
  try { await chmod(path, 0o600); }
  catch (error) { for (const socket of clients) socket.destroy(); await new Promise<void>(accept => server.close(() => accept())); throw error; }
  server.on("error", () => { for (const socket of clients) socket.destroy(); server.close(); });
  return async () => { for (const socket of clients) socket.destroy(); await new Promise<void>(accept => server.close(() => accept())); };
}
