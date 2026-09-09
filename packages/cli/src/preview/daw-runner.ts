import type { DocumentRequest, PreviewFrame } from "@oxitone/protocol";
import { ProjectDocument } from "../source/project-document.js";
import { DocumentDispatcher } from "../source/document-dispatch.js";
import { watchProjectDocument } from "../source/document-watch.js";
import { openDocumentBridge } from "../source/document-bridge.js";

/** GPUI projects observe this owner; all note and code commands share one document transaction queue. */
export class DawRunner {
  private document?: ProjectDocument;
  private dispatcher?: DocumentDispatcher;
  private unsubscribe?: () => void;
  private stopWatching?: () => void;
  private poll?: NodeJS.Timeout;
  private revision = -1;
  private closeBridge?: () => Promise<void>;
  private closed = false;
  constructor(private readonly entry: string, private readonly send: (frame: PreviewFrame) => void,
    private readonly options: { watch?: boolean; watchPaths?: string[]; documentSocket?: string } = {}) {}
  async start(): Promise<void> {
    this.document = await ProjectDocument.open({ entry: this.entry, readPaths: this.options.watchPaths ?? [] });
    this.dispatcher = new DocumentDispatcher(this.document);
    if (this.options.documentSocket) {
      this.closeBridge = await openDocumentBridge(this.options.documentSocket, this.document, this.dispatcher);
      console.error(`Oxitone document bridge · ${this.options.documentSocket}`);
    }
    this.unsubscribe = this.document.subscribe(view => {
      if (view.acceptedRevision !== this.revision && this.document?.frame) {
        this.revision = view.acceptedRevision; this.send(this.document.frame);
      }
      this.send({ protocolVersion: "1.0", type: "document", message: { documentProtocolVersion: "2.0", type: "event", view } });
    });
    if (this.options.watch !== false) this.stopWatching = watchProjectDocument(this.document, this.entry,
      error => this.send({ protocolVersion: "1.0", type: "diagnostic", code: "SourceChanged", message: String(error) }));
    // Reverse requests ride correlated native replies. Presentation events never count as command responses.
    this.poll = setInterval(() => this.send({ protocolVersion: "1.0", type: "query" }), 50);
  }
  receive(requests: readonly DocumentRequest[]): void {
    if (this.closed) return;
    for (const request of requests) void Promise.resolve().then(() => this.closed ? undefined : this.dispatcher?.submit(request)).then(message => {
      if (!this.closed && message) this.send({ protocolVersion: "1.0", type: "document", message });
    }).catch(error => {
      if (!this.closed) this.send({ protocolVersion: "1.0", type: "diagnostic", code: "DraftInvalid", message: String(error) });
    });
  }
  rejectRevision(): void { this.revision = -1; }
  async close(): Promise<void> {
    this.closed = true;
    if (this.poll) clearInterval(this.poll);
    this.stopWatching?.(); this.unsubscribe?.(); this.dispatcher?.close(); this.document?.close();
    await this.closeBridge?.();
  }
}
