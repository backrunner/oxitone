import { createHash } from "node:crypto";
import * as vscode from "vscode";

export const scheme = "oxitone";
export function connectionId(socket: string): string { return createHash("sha256").update(socket).digest("hex").slice(0, 24); }
export function sourceUri(id: string, fileName: string): vscode.Uri {
  return vscode.Uri.from({ scheme, authority: id, path: vscode.Uri.file(fileName).path });
}
export function sourcePath(uri: vscode.Uri): string { return vscode.Uri.from({ scheme: "file", path: uri.path }).fsPath; }
