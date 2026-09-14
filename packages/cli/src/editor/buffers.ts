import { ErrorCode, OxitoneError, type DocumentView } from "@oxitone/protocol";

export interface EditorBufferInput {
  fileName: string;
  text: string;
  version: number;
  baselineText: string;
  baselineSessionId?: string;
}
export interface EditorBuffer extends EditorBufferInput {
  remoteText?: string;
  remoteRevision?: number;
  remoteSessionId?: string;
  conflict?: {
    baseline: string;
    local: string;
    remote: string;
    revision: number;
    version: number;
    reason: "concurrentEdit" | "sessionChanged";
  };
  apply?: { text: string; expectedVersion: number; revision: number; sessionId: string };
}

/** Tracks actual editor buffers; remote proposals need an editor-version acknowledgement. */
export class EditorBuffers {
  readonly values = new Map<string, EditorBuffer>();
  open(input: EditorBufferInput): void {
    if (this.values.has(input.fileName))
      throw new OxitoneError(ErrorCode.EditScopeConflict, "editor buffer is already open");
    this.check(input.fileName, input.text, input.version, input.baselineText);
    if (Buffer.byteLength(input.baselineText) > 8 * 1024 * 1024)
      throw new OxitoneError(ErrorCode.BudgetExceeded, "editor baseline exceeds 8 MiB");
    this.values.set(input.fileName, { ...input });
  }
  change(fileName: string, text: string, version: number): void {
    const buffer = this.require(fileName);
    if (version <= buffer.version) throw new OxitoneError(ErrorCode.SourceChanged, "editor buffer version is stale");
    this.check(fileName, text, version, buffer.baselineText);
    if (text === buffer.apply?.text) {
      buffer.baselineText = text;
      buffer.baselineSessionId = buffer.apply.sessionId;
    }
    buffer.text = text;
    buffer.version = version;
    delete buffer.apply;
  }
  reconcile(view: DocumentView, sent?: { fileName: string; text: string }): void {
    const files = new Map(view.files.map((file) => [file.path, file.text]));
    for (const buffer of this.values.values()) {
      const remote = files.get(buffer.fileName);
      if (remote === undefined) {
        delete buffer.remoteText;
        delete buffer.apply;
        continue;
      }
      buffer.remoteText = remote;
      buffer.remoteRevision = view.revision;
      buffer.remoteSessionId = view.sessionId;
      if (sent?.fileName === buffer.fileName && sent.text === remote)
        this.acknowledge(sent.fileName, sent.text, view.sessionId);
      // Once offered to an asynchronous editor API, keep that exact proposal until its version is acknowledged.
      if (buffer.apply && buffer.apply.expectedVersion === buffer.version) {
        if (buffer.apply.text === remote && buffer.apply.sessionId === view.sessionId)
          buffer.apply.revision = view.revision;
        continue;
      }
      if (buffer.text === remote) {
        this.acknowledge(buffer.fileName, remote, view.sessionId);
        delete buffer.conflict;
        delete buffer.apply;
      } else if (buffer.baselineText === remote) {
        buffer.baselineSessionId = view.sessionId;
        delete buffer.conflict;
        delete buffer.apply;
      } else if (buffer.baselineSessionId && buffer.baselineSessionId !== view.sessionId) {
        buffer.conflict = {
          baseline: buffer.baselineText,
          local: buffer.text,
          remote,
          revision: view.revision,
          version: buffer.version,
          reason: "sessionChanged",
        };
        delete buffer.apply;
      } else if (buffer.text === buffer.baselineText) {
        buffer.apply = {
          text: remote,
          expectedVersion: buffer.version,
          revision: view.revision,
          sessionId: view.sessionId,
        };
        delete buffer.conflict;
      } else {
        buffer.conflict = {
          baseline: buffer.baselineText,
          local: buffer.text,
          remote,
          revision: view.revision,
          version: buffer.version,
          reason: "concurrentEdit",
        };
        delete buffer.apply;
      }
    }
  }
  /** A correlated acceptance establishes a common baseline even if a newer event was already received. */
  acknowledge(fileName: string, text: string, sessionId: string): void {
    const buffer = this.values.get(fileName);
    if (buffer) {
      buffer.baselineText = text;
      buffer.baselineSessionId = sessionId;
    }
  }
  applied(fileName: string, expectedVersion: number, version: number, text: string): void {
    const buffer = this.require(fileName);
    if (buffer.version !== expectedVersion || buffer.apply?.text !== text || version <= expectedVersion)
      throw new OxitoneError(ErrorCode.SourceChanged, "editor changed before remote text could be applied");
    this.change(fileName, text, version);
  }
  resolve(fileName: string, version: number, revision: number, choice: "local" | "remote"): void {
    if (choice !== "local" && choice !== "remote")
      throw new OxitoneError(ErrorCode.EditScopeConflict, "invalid editor conflict resolution");
    const buffer = this.require(fileName),
      conflict = buffer.conflict;
    if (
      !conflict ||
      conflict.version !== version ||
      buffer.version !== version ||
      conflict.revision !== revision ||
      buffer.remoteRevision !== revision
    )
      throw new OxitoneError(ErrorCode.SourceChanged, "editor conflict changed before resolution");
    buffer.baselineText = conflict.remote;
    buffer.baselineSessionId = buffer.remoteSessionId!;
    delete buffer.conflict;
    if (choice === "remote")
      buffer.apply = { text: conflict.remote, expectedVersion: version, revision, sessionId: buffer.remoteSessionId! };
  }
  private require(fileName: string): EditorBuffer {
    const value = this.values.get(fileName);
    if (!value) throw new OxitoneError(ErrorCode.EditTargetMissing, "editor buffer is not open");
    return value;
  }
  private check(fileName: string, text: string, version: number, baseline: string): void {
    if (!Number.isSafeInteger(version) || version < 0)
      throw new OxitoneError(ErrorCode.SourceChanged, "invalid editor buffer version");
    if (
      Buffer.byteLength(text) > 8 * 1024 * 1024 ||
      (!this.values.has(fileName) && this.values.size >= 4096) ||
      [...this.values.values()].reduce(
        (sum, buffer) =>
          sum +
          (buffer.fileName === fileName ? 0 : Buffer.byteLength(buffer.text) + Buffer.byteLength(buffer.baselineText)),
        Buffer.byteLength(text) + Buffer.byteLength(baseline),
      ) >
        32 * 1024 * 1024
    ) {
      throw new OxitoneError(ErrorCode.BudgetExceeded, "editor buffer budget exceeded");
    }
  }
}
