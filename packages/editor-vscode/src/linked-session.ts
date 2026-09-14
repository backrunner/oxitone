import * as vscode from "vscode";
import { EditorDocumentSession, type EditorSessionView } from "@oxitone/cli/editor";
import { sourcePath, sourceUri } from "./uris";
import { textChange } from "./text-change";

export interface SessionMemory {
  socket: string;
  baselines: Record<string, string>;
  baselineSessions?: Record<string, string>;
}

/** The only source text model is the Document Service; VS Code owns actual editor versions. */
export class LinkedSession implements vscode.Disposable {
  readonly editor: EditorDocumentSession;
  view: EditorSessionView;
  private readonly applying = new Set<string>();
  private readonly opened = new Set<string>();
  private readonly served = new Map<string, string>();
  private readonly failures = new Map<string, Error>();
  private readonly subscriptions: vscode.Disposable[] = [];
  private readonly unsubscribe: () => void;
  private saveQueue: Promise<void> = Promise.resolve();
  private disposed = false;
  constructor(
    readonly id: string,
    readonly memory: SessionMemory,
    private readonly changed: () => void,
  ) {
    this.editor = new EditorDocumentSession(memory.socket);
    this.view = this.editor.view;
    this.subscriptions.push(
      vscode.workspace.onDidOpenTextDocument((document) => this.observe(document)),
      vscode.workspace.onDidChangeTextDocument((event) => this.observe(event.document)),
      vscode.workspace.onDidCloseTextDocument((document) => {
        if (!this.owns(document.uri)) return;
        const path = sourcePath(document.uri);
        const buffer = this.editor.view.buffers.find((buffer) => buffer.fileName === path);
        const retainBaseline = document.isDirty || (buffer && buffer.text !== buffer.remoteText);
        this.opened.delete(path);
        this.served.delete(path);
        this.failures.delete(path);
        this.editor.closeBuffer(path);
        if (!retainBaseline) {
          delete memory.baselines[path];
          if (memory.baselineSessions) delete memory.baselineSessions[path];
        }
        this.changed();
      }),
    );
    this.unsubscribe = this.editor.subscribe((view) => {
      this.view = view;
      for (const document of vscode.workspace.textDocuments) this.observe(document);
      for (const buffer of view.buffers) {
        memory.baselines[buffer.fileName] = buffer.baselineText;
        if (buffer.baselineSessionId) (memory.baselineSessions ??= {})[buffer.fileName] = buffer.baselineSessionId;
        if (buffer.apply) void this.apply(buffer.fileName, buffer.apply);
      }
      this.changed();
    });
  }
  owns(uri: vscode.Uri): boolean {
    return uri.scheme === "oxitone" && uri.authority === this.id;
  }
  get failure(): Error | undefined {
    return this.failures.values().next().value;
  }
  read(fileName: string, text: string): void {
    if (!this.opened.has(fileName)) this.served.set(fileName, text);
  }
  private document(fileName: string): vscode.TextDocument | undefined {
    return vscode.workspace.textDocuments.find(
      (document) => this.owns(document.uri) && sourcePath(document.uri) === fileName && !document.isClosed,
    );
  }
  private open(document: vscode.TextDocument): void {
    if (!this.owns(document.uri) || document.isClosed) return;
    const fileName = sourcePath(document.uri);
    if (this.opened.has(fileName)) return;
    const remote = this.view.document?.files.find((file) => file.path === fileName);
    if (!remote) return;
    const readBaseline = this.served.get(fileName) ?? remote.text;
    const restored = document.isDirty && this.memory.baselines[fileName] !== undefined;
    this.editor.openBuffer({
      fileName,
      text: document.getText(),
      version: document.version,
      baselineText: restored ? this.memory.baselines[fileName]! : readBaseline,
      ...(restored
        ? { baselineSessionId: this.memory.baselineSessions?.[fileName] ?? "unknown-restored-session" }
        : {}),
    });
    this.opened.add(fileName);
  }
  private change(document: vscode.TextDocument): void {
    if (!this.owns(document.uri) || document.isClosed) return;
    this.open(document);
    const path = sourcePath(document.uri);
    const buffer = this.editor.view.buffers.find((buffer) => buffer.fileName === path);
    // The change event also acknowledges successful remote WorkspaceEdits. Echoes are not resubmitted.
    if (buffer && document.version > buffer.version)
      this.editor.changeBuffer(path, document.getText(), document.version);
  }
  private observe(document: vscode.TextDocument): void {
    if (!this.owns(document.uri)) return;
    const path = sourcePath(document.uri);
    try {
      this.change(document);
      this.failures.delete(path);
    } catch (error) {
      this.failures.set(path, error instanceof Error ? error : new Error(String(error)));
      this.changed();
    }
  }
  private async apply(fileName: string, proposal: { text: string; expectedVersion: number }): Promise<void> {
    if (this.applying.has(fileName) || this.disposed) return;
    const document = this.document(fileName);
    if (!document || document.version !== proposal.expectedVersion) return;
    this.applying.add(fileName);
    try {
      const edit = new vscode.WorkspaceEdit();
      const change = textChange(document.getText(), proposal.text);
      edit.replace(
        document.uri,
        new vscode.Range(document.positionAt(change.start), document.positionAt(change.end)),
        change.text,
        { label: "Apply Oxitone DAW change", needsConfirmation: false },
      );
      // VS Code's WorkspaceEdit carries the tracked document version to the main-thread bulk edit.
      // Metadata also bypasses VS Code 1.96's async minimal-edit pass, which discards versionId.
      // Build and submit synchronously; typing before application rejects the edit atomically.
      const applied = await vscode.workspace.applyEdit(edit);
      if (applied && !document.isClosed) this.observe(document);
    } catch (error) {
      this.failures.set(fileName, error instanceof Error ? error : new Error(String(error)));
      this.changed();
    } finally {
      this.applying.delete(fileName);
      const next = this.editor.view.buffers.find((buffer) => buffer.fileName === fileName)?.apply;
      if (next && (next.text !== proposal.text || next.expectedVersion !== proposal.expectedVersion))
        void this.apply(fileName, next);
    }
  }
  async ready(): Promise<void> {
    if (this.view.state === "connected") return;
    await new Promise<void>((accept, reject) => {
      const timer = setTimeout(() => {
        unsubscribe();
        reject(new Error("DAW connection timed out. Check the document socket path."));
      }, 15_000);
      const unsubscribe = this.editor.subscribe((view) => {
        if (view.state === "connected" || view.state === "closed") {
          clearTimeout(timer);
          queueMicrotask(() => unsubscribe());
          if (view.state === "connected") accept();
          else reject(new Error("DAW connection closed"));
        }
      });
    });
  }
  async flush(): Promise<void> {
    // Remote application is asynchronous in the host. Give only existing applications time to finish.
    const deadline = Date.now() + 5000;
    while (this.applying.size && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 10));
    if (this.failure) throw this.failure;
    for (const document of vscode.workspace.textDocuments) {
      if (!this.owns(document.uri) || document.isClosed) continue;
      const buffer = this.editor.view.buffers.find((buffer) => buffer.fileName === sourcePath(document.uri));
      if (!buffer || buffer.version !== document.version || buffer.text !== document.getText())
        throw new Error("The actual editor text has not synchronized. Resolve its diagnostic before saving.");
    }
    await this.editor.flush();
  }
  save(fileName?: string, content?: string): Promise<void> {
    const operation = this.saveQueue.then(async () => {
      const document = fileName ? this.document(fileName) : undefined;
      const version = document?.version;
      if (fileName && (!document || document.getText() !== content))
        throw new Error("Save text is stale. Retry saving the current linked editor.");
      await this.flush();
      if (this.view.document?.modified) await this.editor.command("save");
      if (document && (document.version !== version || document.getText() !== content))
        throw new Error("The editor changed during Save; newer typing remains unsaved.");
      if (this.view.document?.status !== "ready")
        throw new Error("Resolve the project diagnostic or conflict before saving.");
    });
    this.saveQueue = operation.catch(() => {});
    return operation;
  }
  async openSource(fileName: string): Promise<vscode.TextEditor> {
    await this.ready();
    return vscode.window.showTextDocument(await vscode.workspace.openTextDocument(sourceUri(this.id, fileName)), {
      preview: false,
    });
  }
  async createFile(fileName: string, text = ""): Promise<void> {
    await this.ready();
    const response = await this.editor.request({ kind: "createFile", fileName, text });
    if (!response.accepted) throw new Error(response.error?.message ?? "The DAW rejected the new source file");
    if (!this.view.document?.files.some((file) => file.path === fileName)) {
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => {
          unsubscribe();
          reject(new Error("The DAW did not publish the new source file"));
        }, 5000);
        const unsubscribe = this.editor.subscribe((view) => {
          if (view.document?.files.some((file) => file.path === fileName)) {
            clearTimeout(timer);
            queueMicrotask(() => unsubscribe());
            resolve();
          }
        });
      });
    }
  }
  async installPlugin(packageName: string, version?: string): Promise<void> {
    await this.ready();
    const response = await this.editor.request({
      kind: "installPlugin",
      packageName,
      ...(version === undefined ? {} : { version }),
    });
    if (!response.accepted) throw new Error(response.error?.message ?? "The DAW rejected the plugin installation");
  }
  dispose(): void {
    this.disposed = true;
    this.unsubscribe();
    this.editor.close();
    for (const disposable of this.subscriptions) disposable.dispose();
  }
}
