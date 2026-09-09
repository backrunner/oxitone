# Oxitone for VS Code

Edit TypeScript and the GPUI DAW through the same live Document Service. No source IDs or binding annotations are required.

1. Install the development VSIX (`pnpm --filter oxitone-vscode package` produces `target/oxitone-vscode.vsix`). Use **Extensions: Install from VSIX** in VS Code.
2. Open a trusted local project with Oxitone installed, select its entry `.ts`, and run **Oxitone: Open Project in DAW**. The default launcher is `pnpm exec oxitone`; configure `oxitone.executable` and `oxitone.arguments` for another local CLI.
3. Edit the sources in the **Oxitone Project** explorer. These linked TypeScript tabs synchronize unsaved text with the DAW. **Cmd+S**, **Save All**, and auto save persist through the project's transaction journal.

You can also run `oxitone daw song.ts` in a terminal, then use **Oxitone: Connect to DAW Session** with the printed document socket path. Multiple sessions are supported. The server and extension must run on the same machine; the current GPUI application targets macOS.

**Oxitone: Undo Project Transaction** and **Redo Project Transaction** use the DAW's shared history (`Cmd+Alt+Z` / `Cmd+Alt+Shift+Z` on macOS). Normal editor Undo retains VS Code's text editing behavior and synchronizes the resulting text as a new project transaction.

Closing a linked tab does not undo transactions already synchronized to the DAW. Use project Undo to discard a transaction or Save to persist the project. The status bar distinguishes source and accepted revisions.

Concurrent edits keep both versions. **Oxitone: Review Editor Conflict** opens a DAW/editor diff and offers the common base before choosing a version. A choice expires when either version changes. Disconnection retains unsaved text; reconnecting the same socket reconciles it before submission. Invalid source remains editable with the last accepted DAW graph; the current server requires valid source for journal Save.

Only the linked `oxitone:` tabs participate in unsaved synchronization and journal Save. Ordinary filesystem tabs continue to use disk watching. Save those tabs before starting a DAW session and edit the linked tabs while it runs. Source import/export statements remain normal TypeScript; evaluation and npm resolution run in the project's Node service. The linked scheme provides TypeScript highlighting; full TypeScript language-service navigation across npm dependencies is not yet integrated.

**Oxitone: Create TypeScript Source File** creates a linked `.ts` or `.mts` module relative to the service's project root. Its parent directory must already exist. Other linked modules can import or re-export it before Save; `.js` imports resolve to `.ts` and `.mjs` to `.mts`. Creating a file joins the shared Undo/Redo history, and project Save persists its creation or removal after Undo.

**Oxitone: Install Plugin Package** accepts an npm package name and optional version. Installation runs through the Document Service. Plugin discovery and materialization review are available in GPUI; materializing notes or effect chains writes local project code and preserves dependency source.

The service restricts source writes to enrolled project roots. General rename/delete, new directories, Save As to a new project, and asset management are not implemented. Session endpoints and common source baselines are stored in VS Code's local workspace state for recovery; source is not uploaded.

Development: build the workspace dependencies and `cargo build -p oxitone-preview`, then `pnpm --filter oxitone-vscode build`. `pnpm --filter oxitone-vscode test:host` downloads an isolated VS Code 1.96.4 test host and exercises real editor buffers, auto save, reconnection and the production launch command. Its GPUI runs headless with a simulated sink; it never opens an audio device. Bundle/VSIX output belongs in ignored `dist`/`target`; no marketplace publishing is performed.
