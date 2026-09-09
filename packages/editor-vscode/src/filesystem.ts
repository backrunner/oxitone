import * as vscode from "vscode";
import type { LinkedSession } from "./linked-session";
import { sourcePath } from "./uris";

/** Linked TS files are served from the Document Service; disk writes remain journaled there. */
export class SourceFileSystem implements vscode.FileSystemProvider, vscode.Disposable {
  private readonly changes = new vscode.EventEmitter<vscode.FileChangeEvent[]>();
  readonly onDidChangeFile = this.changes.event;
  constructor(private readonly sessions: ReadonlyMap<string, LinkedSession>) {}
  watch(): vscode.Disposable { return new vscode.Disposable(() => {}); }
  private session(uri: vscode.Uri): LinkedSession {
    const session = this.sessions.get(uri.authority);
    if (!session || !session.owns(uri)) throw vscode.FileSystemError.Unavailable("Connect to this Oxitone DAW session first.");
    return session;
  }
  private async text(uri: vscode.Uri): Promise<string> {
    const session = this.session(uri), path = sourcePath(uri);
    // Reading the last known baseline allows VS Code to restore its own dirty backups offline.
    if (session.view.state !== "connected") {
      const cached = session.view.document?.files.find(file => file.path === path)?.text ?? session.memory.baselines[path];
      if (cached !== undefined) return cached;
      await session.ready();
    }
    const file = session.view.document?.files.find(file => file.path === path);
    if (!file) throw vscode.FileSystemError.FileNotFound(uri);
    return file.text;
  }
  async stat(uri: vscode.Uri): Promise<vscode.FileStat> {
    return { type: vscode.FileType.File, ctime: 0, mtime: 0, size: Buffer.byteLength(await this.text(uri)) };
  }
  async readFile(uri: vscode.Uri): Promise<Uint8Array> {
    const text = await this.text(uri);
    this.session(uri).read(sourcePath(uri), text);
    return Buffer.from(text);
  }
  async writeFile(uri: vscode.Uri, content: Uint8Array): Promise<void> {
    const session = this.session(uri);
    if (!session.view.document?.files.some(file => file.path === sourcePath(uri))) throw vscode.FileSystemError.NoPermissions("Only existing project sources can be saved.");
    try { await session.save(sourcePath(uri), new TextDecoder("utf-8", { fatal: true }).decode(content)); }
    catch (error) { throw vscode.FileSystemError.Unavailable(String(error)); }
  }
  readDirectory(): [string, vscode.FileType][] { return []; }
  createDirectory(): never { throw vscode.FileSystemError.NoPermissions("Create project files on disk, then reopen the DAW."); }
  delete(): never { throw vscode.FileSystemError.NoPermissions("Linked sources cannot be deleted from the editor."); }
  rename(): never { throw vscode.FileSystemError.NoPermissions("Linked sources cannot be renamed from the editor."); }
  dispose(): void { this.changes.dispose(); }
}
