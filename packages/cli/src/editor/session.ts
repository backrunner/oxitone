import { resolve } from "node:path";
import { ErrorCode, ERROR_CODES, OxitoneError, type DocumentView } from "@oxitone/protocol";
import { EditorBuffers, type EditorBuffer, type EditorBufferInput } from "./buffers.js";
import { EditorConnection, type EditorConnectionState } from "./connection.js";

export interface EditorSessionView {
  state: EditorConnectionState;
  document?: DocumentView;
  buffers: EditorBuffer[];
  submitting: boolean;
  error?: { code: string; message: string };
}
type Sent = { fileName: string; text: string; revision: number; sessionId: string };

/** An editor adapter supplies actual buffer versions and applies remote proposals with version checks. */
export class EditorDocumentSession {
  private readonly buffers = new EditorBuffers();
  private readonly connection: EditorConnection;
  private readonly listeners = new Set<() => void>();
  private sent: Sent | undefined;
  private error: OxitoneError | undefined;
  private scheduled = false;
  private commanding = false;
  private executing = false;
  private previousState: EditorConnectionState = "connecting";
  constructor(socketPath: string) {
    this.connection = new EditorConnection(socketPath, () => this.schedule());
  }
  get view(): EditorSessionView {
    return structuredClone({
      state: this.connection.state,
      buffers: [...this.buffers.values.values()],
      submitting: !!this.sent || this.commanding,
      ...(this.connection.view ? { document: this.connection.view } : {}),
      ...(this.error ? { error: { code: this.error.code, message: this.error.message } } : {}),
    });
  }
  subscribe(listener: (view: EditorSessionView) => void): () => void {
    const notify = () => {
      try {
        listener(this.view);
      } catch {
        this.listeners.delete(notify);
      }
    };
    this.listeners.add(notify);
    notify();
    return () => {
      this.listeners.delete(notify);
    };
  }
  openBuffer(input: EditorBufferInput): void {
    this.buffers.open({ ...input, fileName: resolve(input.fileName) });
    this.schedule();
  }
  changeBuffer(fileName: string, text: string, version: number): void {
    this.buffers.change(resolve(fileName), text, version);
    this.error = undefined;
    this.schedule();
  }
  appliedRemote(fileName: string, expectedVersion: number, version: number, text: string): void {
    this.buffers.applied(resolve(fileName), expectedVersion, version, text);
    this.schedule();
  }
  closeBuffer(fileName: string): void {
    this.buffers.values.delete(resolve(fileName));
    this.schedule();
  }
  resolveConflict(fileName: string, version: number, revision: number, choice: "local" | "remote"): void {
    this.buffers.resolve(resolve(fileName), version, revision, choice);
    this.error = undefined;
    this.schedule();
  }
  private schedule(): void {
    if (this.scheduled) return;
    this.scheduled = true;
    queueMicrotask(() => {
      this.scheduled = false;
      this.synchronize();
    });
  }
  private synchronize(): void {
    const state = this.connection.state;
    if (state === "connected" && this.previousState !== state) this.error = undefined;
    this.previousState = state;
    if (this.connection.view) this.buffers.reconcile(this.connection.view, this.sent);
    for (const listener of this.listeners) listener();
    this.pump();
  }
  private pump(): void {
    const view = this.connection.view;
    if (
      this.executing ||
      this.sent ||
      this.error ||
      this.connection.state !== "connected" ||
      !view ||
      !["ready", "invalid"].includes(view.status) ||
      view.saving
    )
      return;
    const buffer = [...this.buffers.values.values()].find(
      (buffer) =>
        !buffer.apply && !buffer.conflict && buffer.remoteText !== undefined && buffer.text !== buffer.remoteText,
    );
    if (!buffer) return;
    const sent = (this.sent = {
      fileName: buffer.fileName,
      text: buffer.text,
      revision: view.revision,
      sessionId: view.sessionId,
    });
    this.schedule();
    void this.connection
      .request({ kind: "code", fileName: sent.fileName, text: sent.text })
      .then((response) => {
        if (response.accepted) this.buffers.acknowledge(sent.fileName, sent.text, sent.sessionId);
        if (!response.accepted) {
          const code = ERROR_CODES.find((code) => code === response.error?.code) ?? ErrorCode.DraftInvalid;
          if (code !== ErrorCode.SourceChanged || this.connection.view?.revision === sent.revision)
            this.error = new OxitoneError(code, response.error?.message ?? "editor change was rejected");
        }
      })
      .catch((error) => {
        this.error = OxitoneError.isOxitoneError(error)
          ? error
          : new OxitoneError(ErrorCode.SourceChanged, String(error));
      })
      .finally(() => {
        if (this.connection.view) this.buffers.reconcile(this.connection.view, sent);
        this.sent = undefined;
        this.schedule();
      });
  }
  /** Wait for all local text to reach the document; no disk writes happen here. */
  async flush(): Promise<void> {
    this.schedule();
    await Promise.resolve();
    await new Promise<void>((accept, reject) => {
      const timer = setTimeout(
        () => finish(new OxitoneError(ErrorCode.SourceChanged, "editor synchronization timed out")),
        15_000,
      );
      const finish = (error?: Error) => {
        clearTimeout(timer);
        this.listeners.delete(check);
        if (error) reject(error);
        else accept();
      };
      const check = () => {
        if (this.connection.state !== "connected")
          return finish(new OxitoneError(ErrorCode.SourceChanged, "editor is disconnected"));
        if (this.error) return finish(this.error);
        const buffers = [...this.buffers.values.values()].filter(
          (buffer) =>
            !(
              buffer.remoteText === undefined &&
              buffer.remoteSessionId === this.connection.view?.sessionId &&
              buffer.text === buffer.baselineText
            ),
        );
        if (buffers.some((buffer) => buffer.remoteText === undefined))
          return finish(new OxitoneError(ErrorCode.EditTargetMissing, "an editor buffer is outside this document"));
        if (buffers.some((buffer) => buffer.conflict))
          return finish(new OxitoneError(ErrorCode.EditScopeConflict, "resolve editor conflicts before continuing"));
        if (buffers.some((buffer) => buffer.apply))
          return finish(
            new OxitoneError(ErrorCode.SourceChanged, "apply pending remote text in the editor before continuing"),
          );
        if (!this.sent && buffers.every((buffer) => buffer.text === buffer.remoteText)) finish();
      };
      this.listeners.add(check);
      check();
    });
  }
  async command(kind: "save" | "undo" | "redo"): Promise<void> {
    const response = await this.request({ kind });
    if (!response.accepted)
      throw new OxitoneError(
        ERROR_CODES.find((code) => code === response.error?.code) ?? ErrorCode.DraftInvalid,
        response.error?.message ?? "document command was rejected",
      );
  }
  async request(
    operation: import("@oxitone/protocol").DocumentRequest["operation"],
  ): Promise<{ accepted: boolean; error?: { code: string; message: string } | undefined }> {
    if (this.commanding) throw new OxitoneError(ErrorCode.SourceChanged, "another editor command is pending");
    this.commanding = true;
    this.schedule();
    try {
      await this.flush();
      this.executing = true;
      return await this.connection.request(operation);
    } finally {
      this.executing = false;
      this.commanding = false;
      this.schedule();
    }
  }
  close(): void {
    this.connection.close();
    for (const listener of this.listeners) listener();
    this.listeners.clear();
  }
}
