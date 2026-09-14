import { randomUUID } from "node:crypto";
import { connect, type Socket } from "node:net";
import {
  documentMessageSchema,
  ErrorCode,
  OxitoneError,
  type DocumentMessage,
  type DocumentRequest,
  type DocumentView,
} from "@oxitone/protocol";
import { encodeFrame, FrameDecoder } from "../preview/framing.js";

type Response = Extract<DocumentMessage, { type: "response" }>;
type Pending = { accept: (value: Response) => void; reject: (error: Error) => void; timer: NodeJS.Timeout };
export type EditorConnectionState = "connecting" | "connected" | "disconnected" | "closed";

/** Reconnectable control transport. Music commands are never replayed after disconnect. */
export class EditorConnection {
  private socket: Socket | undefined;
  private retry: NodeJS.Timeout | undefined;
  private readonly pending = new Map<string, Pending>();
  private closed = false;
  private attempt = 0;
  private currentView: DocumentView | undefined;
  private currentState: EditorConnectionState = "connecting";
  private readonly clientId = randomUUID();
  private sequence = 0;
  constructor(
    private readonly path: string,
    private readonly changed: () => void,
  ) {
    this.open();
  }
  get state(): EditorConnectionState {
    return this.currentState;
  }
  get view(): DocumentView | undefined {
    return this.currentView;
  }
  private open(): void {
    if (this.closed) return;
    this.currentState = "connecting";
    const decoder = new FrameDecoder(),
      socket = (this.socket = connect(this.path));
    const handshake = setTimeout(() => socket.destroy(new Error("document handshake timed out")), 5000);
    socket.on("data", (chunk: Buffer) => {
      try {
        for (const raw of decoder.push(chunk)) {
          const message = documentMessageSchema.parse(raw);
          if (message.type === "event") {
            if (
              this.currentView &&
              message.view.sessionId === this.currentView.sessionId &&
              message.view.revision < this.currentView.revision
            )
              continue;
            if (this.currentView && message.view.sessionId !== this.currentView.sessionId)
              this.rejectPending("document session changed");
            this.currentView = message.view;
            this.currentState = "connected";
            this.attempt = 0;
            clearTimeout(handshake);
            this.changed();
          } else {
            const pending = this.pending.get(message.requestId);
            if (!pending || message.sessionId !== this.currentView?.sessionId) continue;
            clearTimeout(pending.timer);
            this.pending.delete(message.requestId);
            pending.accept(message);
          }
        }
      } catch (error) {
        socket.destroy(error as Error);
      }
    });
    socket.on("error", () => {
      /* Close owns retry and pending failure. */
    });
    socket.on("close", () => {
      clearTimeout(handshake);
      if (this.socket !== socket) return;
      this.socket = undefined;
      this.currentState = this.closed ? "closed" : "disconnected";
      this.rejectPending("document connection closed; refresh before retrying a command");
      this.changed();
      if (!this.closed)
        this.retry = setTimeout(() => this.open(), Math.min(2000, 100 * 2 ** Math.min(this.attempt++, 5)));
    });
  }
  request(operation: DocumentRequest["operation"]): Promise<Response> {
    const view = this.currentView,
      socket = this.socket;
    if (this.currentState !== "connected" || !view || !socket)
      return Promise.reject(new OxitoneError(ErrorCode.SourceChanged, "document is disconnected"));
    if (this.pending.size >= 64 || socket.writableLength > 64 * 1024 * 1024)
      return Promise.reject(new OxitoneError(ErrorCode.BudgetExceeded, "editor request queue is full"));
    if (this.sequence >= Number.MAX_SAFE_INTEGER)
      return Promise.reject(new OxitoneError(ErrorCode.BudgetExceeded, "editor request sequence exhausted"));
    const requestId = `stream/${this.clientId}/${++this.sequence}`;
    const timeout = ["installPlugin", "upgradePlugin", "uninstallPlugin", "repairPlugin"].includes(operation.kind)
      ? 150_000
      : 15_000;
    return new Promise((accept, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(requestId);
        reject(new OxitoneError(ErrorCode.SourceChanged, "document response timed out"));
        socket.destroy();
      }, timeout);
      this.pending.set(requestId, { accept, reject, timer });
      try {
        socket.write(
          encodeFrame({
            documentProtocolVersion: "2.0",
            sessionId: view.sessionId,
            requestId,
            baseRevision: view.revision,
            operation,
          }),
        );
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(requestId);
        reject(error as Error);
      }
    });
  }
  private rejectPending(message: string): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new OxitoneError(ErrorCode.SourceChanged, message));
    }
    this.pending.clear();
  }
  close(): void {
    this.closed = true;
    this.currentState = "closed";
    if (this.retry) clearTimeout(this.retry);
    this.rejectPending("editor connection closed");
    this.socket?.destroy();
    this.changed();
  }
}
