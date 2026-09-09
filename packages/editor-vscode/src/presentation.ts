import { basename } from "node:path";
import * as vscode from "vscode";
import type { LinkedSession } from "./linked-session";
import { sourceUri } from "./uris";

type Item = { session: LinkedSession; fileName?: string };
export class Presentation implements vscode.TreeDataProvider<Item>, vscode.Disposable {
  private readonly changes = new vscode.EventEmitter<Item | undefined>();
  readonly onDidChangeTreeData = this.changes.event;
  private readonly status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 20);
  private readonly diagnostics = vscode.languages.createDiagnosticCollection("oxitone");
  constructor(private readonly sessions: ReadonlyMap<string, LinkedSession>) { this.status.command = "oxitone.openSource"; }
  refresh(): void {
    this.changes.fire(undefined); this.diagnostics.clear();
    const active = vscode.window.activeTextEditor?.document.uri;
    const session = active ? this.sessions.get(active.authority) : undefined;
    const selected = session ?? this.sessions.values().next().value;
    const view = selected?.view;
    if (!view) { this.status.hide(); return; }
    const conflicts = view.buffers.filter(buffer => buffer.conflict).length;
    const state = conflicts ? `${conflicts} editor conflicts` : selected?.failure?.message ?? view.error?.message ?? view.document?.diagnostic?.message
      ?? (view.state !== "connected" ? view.state : view.submitting ? "synchronizing" : view.buffers.some(buffer => buffer.apply) ? "applying DAW changes" : view.document?.status !== "ready" ? view.document?.status : view.document.modified ? "modified" : "saved");
    this.status.text = `$(music) Oxitone: ${(state ?? "connecting").slice(0, 55)}`;
    this.status.tooltip = `${state}\nSource revision ${view.document?.revision ?? "—"}; accepted ${view.document?.acceptedRevision ?? "—"}. Open linked project sources.`;
    this.status.show();
    for (const session of this.sessions.values()) {
      const diagnostic = session.view.document?.diagnostic;
      if (!diagnostic) continue;
      for (const file of session.view.document?.files ?? []) {
        const item = new vscode.Diagnostic(new vscode.Range(0, 0, 0, 1), `[${diagnostic.code}] ${diagnostic.message}`, vscode.DiagnosticSeverity.Error);
        item.source = "Oxitone project";
        this.diagnostics.set(sourceUri(session.id, file.path), [item]);
      }
    }
  }
  getChildren(parent?: Item): Item[] {
    if (parent?.fileName) return [];
    if (parent) return (parent.session.view.document?.files ?? []).map(file => ({ session: parent.session, fileName: file.path }));
    return [...this.sessions.values()].map(session => ({ session }));
  }
  getTreeItem(item: Item): vscode.TreeItem {
    if (!item.fileName) {
      const node = new vscode.TreeItem(basename(item.session.view.document?.files[0]?.path ?? item.session.memory.socket), vscode.TreeItemCollapsibleState.Expanded);
      node.id = item.session.id; node.description = item.session.view.state; node.tooltip = item.session.memory.socket; return node;
    }
    const node = new vscode.TreeItem(sourceUri(item.session.id, item.fileName));
    node.description = item.session.view.buffers.some(buffer => buffer.fileName === item.fileName && buffer.conflict) ? "Conflict" : item.fileName;
    node.command = { command: "oxitone.openSource", title: "Open source", arguments: [item] }; return node;
  }
  dispose(): void { this.status.dispose(); this.changes.dispose(); this.diagnostics.dispose(); }
}

export class ConflictReview implements vscode.TextDocumentContentProvider, vscode.Disposable {
  private readonly snapshots = new Map<string, string>();
  private sequence = 0;
  private snapshot(text: string, label: string): vscode.Uri {
    const uri = vscode.Uri.from({ scheme: "oxitone-review", path: `/${++this.sequence}/${label}.ts` });
    this.snapshots.set(uri.toString(), text);
    while (this.snapshots.size > 24) this.snapshots.delete(this.snapshots.keys().next().value!);
    return uri;
  }
  provideTextDocumentContent(uri: vscode.Uri): string { return this.snapshots.get(uri.toString()) ?? "This review expired. Open a new conflict review."; }
  async review(session: LinkedSession): Promise<void> {
    const conflicts = session.view.buffers.filter(buffer => buffer.conflict);
    const selected = await vscode.window.showQuickPick(conflicts.map(buffer => ({ label: basename(buffer.fileName), description: buffer.fileName, buffer })), { title: "Oxitone editor conflicts" });
    if (!selected) return;
    const buffer = selected.buffer, conflict = buffer.conflict!;
    const remote = this.snapshot(conflict.remote, "DAW"), base = this.snapshot(conflict.baseline, "Base");
    await vscode.commands.executeCommand("vscode.diff", remote, sourceUri(session.id, buffer.fileName), "Oxitone: DAW ↔ Editor");
    let action: string | undefined;
    do {
      action = await vscode.window.showWarningMessage(conflict.reason === "sessionChanged"
        ? "The restarted DAW has different source. Review the difference before replacing your retained editor text."
        : "Both the DAW and editor changed this source. Choose which text to keep after reviewing the difference.", "Keep Editor Text", "Use DAW Text", "View Base");
      if (action === "View Base") await vscode.commands.executeCommand("vscode.diff", base, sourceUri(session.id, buffer.fileName), "Oxitone: Base ↔ Editor");
    } while (action === "View Base");
    if (action) session.editor.resolveConflict(buffer.fileName, conflict.version, conflict.revision, action === "Use DAW Text" ? "remote" : "local");
  }
  dispose(): void { this.snapshots.clear(); }
}
