import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import * as vscode from "vscode";
import type { EditorConnection } from "../../../cli/src/editor/connection";

export async function checkSourceCreation(peer: EditorConnection, entryEditor: vscode.TextEditor, root: string,
  until: (check: () => boolean, label: string) => Promise<void>): Promise<void> {
  const path = join(root, "created.ts");
  await vscode.commands.executeCommand("oxitone.createSourceFile", "created.ts");
  const created = vscode.window.activeTextEditor!;
  assert.equal(created.document.uri.path, path);
  assert.equal(created.document.uri.authority, entryEditor.document.uri.authority);
  assert.equal(peer.view!.projectRoot, root);
  await assert.rejects(stat(path), { code: "ENOENT" });
  // An open empty buffer must not prevent the shared history from undoing and redoing creation.
  await vscode.commands.executeCommand("oxitone.undo");
  await until(() => !peer.view!.files.some(file => file.path === path), "Undo new source while its tab remains open");
  await vscode.commands.executeCommand("oxitone.redo");
  await until(() => peer.view!.files.some(file => file.path === path), "Redo new source while its tab remains open");
  assert(await created.edit(edit => edit.insert(new vscode.Position(0, 0), "import { chord } from '@oxitone/core'; export const melody = chord(60, 'major');\n")));
  await until(() => peer.view!.status === "ready" && peer.view!.files.find(file => file.path === path)!.text.includes("chord(60"), "new source buffer evaluation");
  const entry = await vscode.window.showTextDocument(entryEditor.document, { preview: false });
  assert(await entry.edit(edit => edit.replace(new vscode.Range(entry.document.positionAt(0), entry.document.positionAt(entry.document.getText().length)),
    "import { Project } from '@oxitone/core'; import { melody } from './created.js'; const project = new Project(); project.addTrack('New').pattern(melody).at({ bar: 1 }); export default project;\n")));
  await until(() => peer.view!.status === "ready" && peer.view!.sites.some(site => site.fileName === path), "import unsaved module from linked entry");
  const site = peer.view!.sites.find(site => site.fileName === path && site.label === "melody" && site.scope === "definition")!;
  assert((await peer.request({ kind: "notes", site: site.handle, edits: [{ select: { degree: 2 }, set: { pitch: 65 } }] })).accepted);
  await until(() => created.document.getText().includes(".edit("), "DAW notes update newly created editor buffer");
  await vscode.commands.executeCommand("oxitone.save");
  assert.equal(await readFile(path, "utf8"), created.document.getText());
  assert.equal(await readFile(join(root, "song.ts"), "utf8"), entry.document.getText());
  assert(!peer.view!.modified);
}
