import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import * as vscode from "vscode";
import { EditorConnection } from "@oxitone/cli/editor";
import { checkSourceCreation } from "./source-creation";

async function until(check: () => boolean, label: string): Promise<void> {
  const deadline = Date.now() + 20_000;
  while (!check()) {
    if (Date.now() > deadline) throw new Error(`Timed out: ${label}`);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}
async function start(entry: string, socket: string): Promise<ChildProcess> {
  const child = spawn(process.env.OXITONE_TEST_NODE!, [process.env.OXITONE_TEST_SERVICE!, entry, socket], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  child.stdout!.on("data", (data) => {
    output += String(data);
  });
  child.stderr!.on("data", (data) => {
    output += String(data);
  });
  await until(() => {
    if (child.exitCode !== null) throw new Error(output);
    return output.includes("READY");
  }, "Document Service start");
  return child;
}
async function stop(child: ChildProcess): Promise<void> {
  child.kill("SIGTERM");
  await until(() => child.exitCode !== null || child.signalCode !== null, "Document Service stop");
}

export async function run(): Promise<void> {
  const root = process.env.OXITONE_TEST_ROOT!,
    entry = join(root, "song.ts"),
    socket = join(root, "document");
  const initial = await readFile(entry, "utf8");
  let service = await start(entry, socket);
  let dawTask: vscode.TaskExecution | undefined;
  const taskMonitor = vscode.tasks.onDidStartTask((event) => {
    if (event.execution.task.name === "DAW") dawTask = event.execution;
  });
  const peer = new EditorConnection(socket, () => {});
  try {
    await vscode.extensions.getExtension("oxitone.oxitone-vscode")!.activate();
    await vscode.commands.executeCommand("oxitone.connect", socket);
    let editor = vscode.window.activeTextEditor!;
    assert.equal(editor.document.uri.scheme, "oxitone");
    assert.equal(editor.document.languageId, "typescript");
    assert.equal(editor.document.getText(), initial);
    const range = () =>
      new vscode.Range(editor.document.positionAt(0), editor.document.positionAt(editor.document.getText().length));
    const typed = initial.replace("chord(60", "chord(64");
    assert(await editor.edit((edit) => edit.replace(range(), typed)));
    await until(() => peer.view?.files[0]?.text === typed && peer.view.status === "ready", "unsaved editor evaluation");
    assert.equal(await readFile(entry, "utf8"), initial);
    assert(editor.document.isDirty);
    const site = peer.view!.sites.find((site) => site.label === "phrase")!;
    assert(
      (await peer.request({ kind: "notes", site: site.handle, edits: [{ select: { degree: 2 }, set: { pitch: 71 } }] }))
        .accepted,
    );
    await until(() => editor.document.getText() === peer.view!.files[0]!.text, "DAW edit in actual VS Code buffer");
    const generated = editor.document.getText();
    assert.notEqual(generated, typed);
    assert.equal(await readFile(entry, "utf8"), initial);
    assert(await editor.document.save());
    assert.equal(await readFile(entry, "utf8"), generated);
    assert(!editor.document.isDirty);
    assert.equal(peer.view!.modified, false);
    const other = await vscode.workspace.openTextDocument({ language: "plaintext", content: "another editor" });
    await vscode.window.showTextDocument(other, { preview: false });
    assert(
      (await peer.request({ kind: "code", fileName: entry, text: `${generated}\n// DAW change while hidden` }))
        .accepted,
    );
    await until(() => editor.document.getText().includes("DAW change while hidden"), "hidden linked buffer update");
    editor = await vscode.window.showTextDocument(editor.document, { preview: false });
    assert((await peer.request({ kind: "undo" })).accepted);
    await until(() => editor.document.getText() === generated, "restore hidden edit");
    await vscode.commands.executeCommand("oxitone.undo");
    await until(() => editor.document.getText() === typed, "project Undo");
    await vscode.commands.executeCommand("oxitone.redo");
    await until(() => editor.document.getText() === generated, "project Redo");
    const stale = new vscode.WorkspaceEdit();
    stale.replace(editor.document.uri, range(), "stale text", {
      label: "Apply Oxitone DAW change",
      needsConfirmation: false,
    });
    const pending = vscode.workspace.applyEdit(stale);
    const newer = editor.edit((edit) => edit.insert(new vscode.Position(0, 0), "// concurrent input\n"));
    const outcomes = await Promise.all([pending, newer]);
    assert(outcomes.includes(false), "one racing edit must reject on actual VS Code document version");
    assert(await editor.edit((edit) => edit.replace(range(), generated)));
    await until(() => peer.view!.status === "ready" && peer.view!.files[0]!.text === generated, "restore valid source");
    const acknowledged = `${generated}\n// acknowledged but not saved before restart`;
    assert(await editor.edit((edit) => edit.replace(range(), acknowledged)));
    await until(
      () => peer.view!.status === "ready" && peer.view!.files[0]!.text === acknowledged,
      "acknowledge unsaved source",
    );
    const priorSession = peer.view!.sessionId;
    await stop(service);
    service = await start(entry, socket);
    await until(
      () => peer.state === "connected" && peer.view!.sessionId !== priorSession,
      "service restarts from older disk source",
    );
    await vscode.commands.executeCommand("oxitone.connect", socket); // Wait for the actual extension to reconnect too.
    await assert.rejects(Promise.resolve(vscode.commands.executeCommand("oxitone.save")), /resolve editor conflicts/);
    assert.equal(editor.document.getText(), acknowledged, "restart must retain acknowledged but unsaved editor text");
    assert.equal(await readFile(entry, "utf8"), generated);
    // An explicit matching edit in the DAW resolves the disagreement without overwriting either editor.
    assert((await peer.request({ kind: "code", fileName: entry, text: acknowledged })).accepted);
    await until(
      () => peer.view!.status === "ready" && peer.view!.files[0]!.text === acknowledged,
      "reconcile matching source",
    );
    assert(await editor.document.save());
    await stop(service);
    await until(() => peer.state !== "connected", "disconnect");
    const offline = `${acknowledged}\n// retained offline`;
    assert(await editor.edit((edit) => edit.replace(range(), offline)));
    assert.equal(await editor.document.save(), false);
    assert.equal(await readFile(entry, "utf8"), acknowledged);
    service = await start(entry, socket);
    await until(
      () => peer.state === "connected" && peer.view!.status === "ready" && peer.view!.files[0]!.text === offline,
      "reconnect dirty editor",
    );
    assert.equal(editor.document.getText(), offline);
    assert(await editor.document.save());
    assert.equal(await readFile(entry, "utf8"), offline);
    const configuration = vscode.workspace.getConfiguration("files", editor.document.uri);
    await configuration.update("autoSave", "afterDelay", vscode.ConfigurationTarget.Workspace);
    await configuration.update("autoSaveDelay", 100, vscode.ConfigurationTarget.Workspace);
    const autoSaved = `${offline}\n// saved through real VS Code auto save`;
    assert(await editor.edit((edit) => edit.replace(range(), autoSaved)));
    await until(
      () => !editor.document.isDirty && peer.view!.files[0]!.text === autoSaved && !peer.view!.modified,
      "auto save through journal",
    );
    assert.equal(await readFile(entry, "utf8"), autoSaved);
    await configuration.update("autoSave", "off", vscode.ConfigurationTarget.Workspace);
    await checkSourceCreation(peer, editor, root, until);
    await assert.rejects(
      Promise.resolve(
        vscode.workspace.fs.writeFile(
          editor.document.uri.with({ path: `${root}/node_modules/evil.ts` }),
          Buffer.from("no"),
        ),
      ),
    );
    const launch = vscode.workspace.getConfiguration("oxitone");
    await launch.update("executable", process.env.OXITONE_TEST_NODE, vscode.ConfigurationTarget.Global);
    await launch.update("arguments", [process.env.OXITONE_TEST_CLI], vscode.ConfigurationTarget.Global);
    await launch.update(
      "previewArguments",
      ["--headless", "--viewer", process.env.OXITONE_TEST_VIEWER],
      vscode.ConfigurationTarget.Global,
    );
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(vscode.Uri.file(entry)), {
      preview: false,
    });
    await vscode.commands.executeCommand("oxitone.start");
    dawTask = vscode.tasks.taskExecutions.find((execution) => execution.task.name === "DAW");
    assert(dawTask, "production DAW task is running");
    const launched = vscode.window.activeTextEditor!;
    assert.equal(launched.document.uri.scheme, "oxitone");
    assert.notEqual(launched.document.uri.authority, editor.document.uri.authority);
    assert(await launched.edit((edit) => edit.insert(new vscode.Position(0, 0), "// CLI launched from VS Code\n")));
    assert(await launched.document.save());
    assert((await readFile(entry, "utf8")).startsWith("// CLI launched from VS Code"));
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    console.log(
      "PASS Oxitone Extension Host: unsaved TS, visible/hidden DAW edits, journal Save/auto save, Undo/Redo, version race, unsaved restart conflict, offline reconnect, dependency guard, production CLI task with simulated GPUI.",
    );
  } finally {
    taskMonitor.dispose();
    dawTask?.terminate();
    peer.close();
    await stop(service);
  }
}
