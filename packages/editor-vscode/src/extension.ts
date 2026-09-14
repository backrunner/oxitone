import { chmod, mkdtemp, rm } from "node:fs/promises";
import { dirname, isAbsolute, join } from "node:path";
import * as vscode from "vscode";
import { SourceFileSystem } from "./filesystem";
import { LinkedSession, type SessionMemory } from "./linked-session";
import { ConflictReview, Presentation } from "./presentation";
import { connectionId, scheme } from "./uris";

export function activate(context: vscode.ExtensionContext): void {
  const sessions = new Map<string, LinkedSession>();
  const presentation = new Presentation(sessions),
    conflicts = new ConflictReview();
  const fileSystem = new SourceFileSystem(sessions);
  let persistence: ReturnType<typeof setTimeout> | undefined;
  const remember = () =>
    context.workspaceState.update(
      "sessions",
      [...sessions.values()].map((session) => session.memory),
    );
  const changed = () => {
    presentation.refresh();
    if (persistence) clearTimeout(persistence);
    persistence = setTimeout(() => {
      void remember();
    }, 100);
  };
  function connect(socket: string, memory: SessionMemory = { socket, baselines: {} }): LinkedSession {
    if (!vscode.workspace.isTrusted) throw new Error("Trust the workspace before evaluating Oxitone source.");
    if (!isAbsolute(socket) || Buffer.byteLength(socket) >= 104)
      throw new Error("Use an absolute local Unix document socket path shorter than 104 bytes.");
    const id = connectionId(socket),
      existing = sessions.get(id);
    if (existing) return existing;
    if (sessions.size >= 8) throw new Error("Disconnect an unused DAW session before opening another (limit: 8).");
    const session = new LinkedSession(id, memory, changed);
    sessions.set(id, session);
    changed();
    return session;
  }
  async function choose(): Promise<LinkedSession | undefined> {
    const active = vscode.window.activeTextEditor?.document.uri;
    if (active?.scheme === scheme && sessions.has(active.authority)) return sessions.get(active.authority);
    if (sessions.size === 1) return sessions.values().next().value;
    const item = await vscode.window.showQuickPick(
      [...sessions.values()].map((session) => ({ label: session.memory.socket, session })),
      { title: "Choose Oxitone session" },
    );
    return item?.session;
  }
  async function openSource(item?: { session: LinkedSession; fileName: string }): Promise<void> {
    if (item?.session && item.fileName) {
      await item.session.openSource(item.fileName);
      return;
    }
    const session = await choose();
    if (!session) return;
    await session.ready();
    const source = await vscode.window.showQuickPick(
      (session.view.document?.files ?? []).map((file) => file.path),
      { title: "Open linked TypeScript source" },
    );
    if (source) await session.openSource(source);
  }
  function command(name: string, action: (...args: never[]) => unknown): void {
    context.subscriptions.push(
      vscode.commands.registerCommand(name, async (...args) => {
        try {
          return await action(...(args as never[]));
        } catch (error) {
          void vscode.window.showErrorMessage(`Oxitone: ${error instanceof Error ? error.message : String(error)}`);
          throw error;
        }
      }),
    );
  }
  command("oxitone.connect", async (socket?: string) => {
    socket ??= await vscode.window.showInputBox({
      title: "Connect to Oxitone DAW",
      prompt: "Document socket path printed by oxitone daw",
      placeHolder: "/tmp/oxitone-preview-…/document",
    });
    if (!socket) return;
    const session = connect(socket);
    await session.ready();
    const first = session.view.document?.files[0];
    if (first) await session.openSource(first.path);
  });
  command("oxitone.start", async () => {
    const active = vscode.window.activeTextEditor?.document;
    const entry =
      active?.uri.scheme === "file" && /\.[cm]?tsx?$/.test(active.fileName)
        ? active.uri
        : (
            await vscode.window.showOpenDialog({
              canSelectMany: false,
              filters: { TypeScript: ["ts", "mts", "cts", "tsx"] },
            })
          )?.[0];
    if (!entry) return;
    if (!vscode.workspace.isTrusted) throw new Error("Trust the workspace before starting a DAW project.");
    if (
      vscode.workspace.textDocuments.some(
        (document) =>
          document.uri.scheme === "file" &&
          document.isDirty &&
          document.uri.fsPath.startsWith(`${dirname(entry.fsPath)}/`),
      )
    ) {
      throw new Error(
        "Save existing file buffers before opening this project in the DAW; then continue in its linked sources.",
      );
    }
    const directory = await mkdtemp("/tmp/oxitone-editor-");
    await chmod(directory, 0o700);
    const socket = join(directory, "document");
    const config = vscode.workspace.getConfiguration("oxitone", entry);
    const execution = new vscode.ProcessExecution(
      config.get<string>("executable", "pnpm"),
      [
        ...config.get<string[]>("arguments", ["exec", "oxitone"]),
        "daw",
        entry.fsPath,
        ...config.get<string[]>("previewArguments", []),
        "--document-socket",
        socket,
      ],
      { cwd: dirname(entry.fsPath) },
    );
    const task = new vscode.Task(
      { type: "oxitone" },
      vscode.workspace.getWorkspaceFolder(entry) ?? vscode.TaskScope.Workspace,
      "DAW",
      "Oxitone",
      execution,
    );
    task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Dedicated };
    const running = await vscode.tasks.executeTask(task);
    const ended = vscode.tasks.onDidEndTaskProcess((event) => {
      if (event.execution !== running) return;
      ended.dispose();
      void rm(directory, { recursive: true, force: true });
    });
    context.subscriptions.push(ended);
    const session = connect(socket);
    await session.ready();
    await session.openSource(entry.fsPath);
  });
  command("oxitone.openSource", openSource);
  command("oxitone.createSourceFile", async (requestedPath?: string) => {
    const session = await choose();
    if (!session) return;
    await session.ready();
    const root = session.view.document?.projectRoot;
    if (!root) throw new Error("The DAW session is disconnected.");
    const input =
      requestedPath ??
      (await vscode.window.showInputBox({
        title: "Create Oxitone TypeScript source",
        prompt: "Path relative to the project source root",
        value: "helpers.ts",
        validateInput: (value) => (/(?<!\.d)\.(?:ts|mts)$/.test(value) ? undefined : "Use a .ts or .mts source file"),
      }));
    if (!input) return;
    const fileName = isAbsolute(input) ? input : join(root, input);
    await session.createFile(fileName);
    await session.openSource(fileName);
  });
  command("oxitone.installPlugin", async () => {
    const session = await choose();
    if (!session) return;
    const input = await vscode.window.showInputBox({
      title: "Install Oxitone Plugin Package",
      prompt: "npm package name, optionally followed by @version",
      placeHolder: "@acme/synth@1.2.3",
    });
    if (!input) return;
    const match = input.match(
      /^(?:(@[a-z0-9][a-z0-9._-]*\/)?)((?:[a-z0-9][a-z0-9._-]*))(?:@([a-zA-Z0-9][a-zA-Z0-9._+~-]*))?$/,
    );
    if (!match) throw new Error("Enter a valid npm package name and optional version");
    await session.installPlugin(`${match[1] ?? ""}${match[2]}`, match[3]);
  });
  command("oxitone.save", async () => {
    const session = await choose();
    if (!session) return;
    await session.save();
    for (const document of vscode.workspace.textDocuments)
      if (session.owns(document.uri) && document.isDirty && !(await document.save()))
        throw new Error("The linked editor could not finish saving.");
  });
  for (const kind of ["undo", "redo"] as const)
    command(`oxitone.${kind}`, async () => {
      const session = await choose();
      if (session) {
        await session.flush();
        await session.editor.command(kind);
      }
    });
  command("oxitone.resolveConflict", async () => {
    const session = await choose();
    if (session) await conflicts.review(session);
  });
  command("oxitone.disconnect", async () => {
    const session = await choose();
    if (!session) return;
    if (
      session.view.document?.modified ||
      vscode.workspace.textDocuments.some((document) => session.owns(document.uri) && document.isDirty)
    )
      throw new Error("Save or resolve pending edits before disconnecting.");
    if (vscode.workspace.textDocuments.some((document) => session.owns(document.uri)))
      throw new Error("Close this session's linked source editors before disconnecting.");
    session.dispose();
    sessions.delete(session.id);
    changed();
  });
  context.subscriptions.push(
    fileSystem,
    presentation,
    conflicts,
    vscode.workspace.registerFileSystemProvider(scheme, fileSystem, { isCaseSensitive: true }),
    vscode.workspace.registerTextDocumentContentProvider("oxitone-review", conflicts),
    vscode.window.registerTreeDataProvider("oxitone.sources", presentation),
    vscode.window.onDidChangeActiveTextEditor(() => presentation.refresh()),
    vscode.window.registerUriHandler({
      handleUri: async (uri) => {
        const socket = new URLSearchParams(uri.query).get("socket");
        if (uri.path === "/connect" && socket) await vscode.commands.executeCommand("oxitone.connect", socket);
      },
    }),
    new vscode.Disposable(() => {
      if (persistence) clearTimeout(persistence);
      void remember();
      for (const session of sessions.values()) session.dispose();
    }),
  );
  if (vscode.workspace.isTrusted)
    for (const memory of context.workspaceState.get<SessionMemory[]>("sessions", [])) {
      try {
        connect(memory.socket, memory);
      } catch {
        /* Invalid cached endpoints remain disconnected. */
      }
    }
}
