import {
  documentRequestSchema,
  ErrorCode,
  OxitoneError,
  type DocumentMessage,
  type DocumentRequest,
} from "@oxitone/protocol";
import type { ProjectDocument } from "./project-document.js";
import { DocumentRequestHistory } from "./document-request-history.js";

type Response = Extract<DocumentMessage, { type: "response" }>;
/** Ordered commit queue with request identity independent of document events or native snapshot acknowledgements. */
export class DocumentDispatcher {
  private queue: Promise<unknown> = Promise.resolve();
  private queued = 0;
  private closed = false;
  private readonly requests = new DocumentRequestHistory();
  constructor(private readonly document: ProjectDocument) {}
  submit(input: unknown): Promise<Response> {
    const request = documentRequestSchema.parse(input);
    const fingerprint = this.requests.fingerprint(request);
    const previous = this.requests.get(request.requestId);
    if (previous)
      return previous.fingerprint === fingerprint
        ? previous.result
        : Promise.resolve(this.reject(request, ErrorCode.SourceChanged, "requestId reused for a different command"));
    if (this.closed || request.sessionId !== this.document.sessionId)
      return Promise.resolve(this.reject(request, ErrorCode.SourceChanged, "document session is no longer current"));
    if (this.queued >= 64)
      return Promise.resolve(this.reject(request, ErrorCode.BudgetExceeded, "document request budget exceeded"));
    try {
      this.requests.reserve(request.requestId);
    } catch (error) {
      return Promise.resolve(
        this.reject(request, OxitoneError.isOxitoneError(error) ? error.code : ErrorCode.SourceChanged, String(error)),
      );
    }
    this.queued++;
    const result = this.queue.then(async () => {
      try {
        if (this.closed) throw new OxitoneError(ErrorCode.SourceChanged, "document connection closed");
        await this.execute(request);
        return {
          documentProtocolVersion: "2.0" as const,
          type: "response" as const,
          sessionId: request.sessionId,
          requestId: request.requestId,
          accepted: true,
          revision: this.document.view.revision,
        };
      } catch (error) {
        return this.reject(
          request,
          OxitoneError.isOxitoneError(error) ? error.code : ErrorCode.DraftInvalid,
          error instanceof Error ? error.message : String(error),
        );
      } finally {
        this.queued--;
      }
    });
    this.requests.remember(request.requestId, { fingerprint, result });
    this.queue = result.then(() => this.requests.settle());
    return result;
  }
  private reject(request: DocumentRequest, code: string, message: string): Response {
    return {
      documentProtocolVersion: "2.0",
      type: "response",
      sessionId: request.sessionId,
      requestId: request.requestId,
      accepted: false,
      revision: this.document.view.revision,
      error: { code, message },
    };
  }
  private async execute(request: DocumentRequest): Promise<void> {
    const operation = request.operation,
      revision = request.baseRevision;
    switch (operation.kind) {
      case "project":
        await this.document.configure(revision, operation.edit);
        break;
      case "assignPlugin":
        await this.document.assignPlugin(revision, operation);
        break;
      case "arrangement":
        await this.document.arrange(revision, operation.edit);
        break;
      case "notes":
        await this.document.edit(revision, operation.site, operation.edits, operation.placement);
        break;
      case "automationRange":
        await this.document.editAutomationRange(
          revision,
          operation.site,
          operation.edit,
          operation.lane,
          operation.clip,
        );
        break;
      case "configuration":
        await this.document.editConfiguration(revision, operation.site, operation.edit, operation.usage);
        break;
      case "effectOrder":
        await this.document.editEffectOrder(revision, operation.site, operation.owner, operation.order);
        break;
      case "planMaterializeRack":
        await this.document.planMaterializeRack(revision, operation.site, operation.owner);
        break;
      case "planMaterialize":
        await this.document.planMaterialize(revision, operation.site, operation.edits, operation.placement);
        break;
      case "confirmMaterialize":
        await this.document.confirmMaterialize(revision, operation.planId);
        break;
      case "cancelMaterialize":
        this.document.cancelMaterialize(revision, operation.planId);
        break;
      case "code":
        await this.document.changeCode(revision, operation.fileName, operation.text);
        break;
      case "createFile":
        await this.document.createFile(revision, operation.fileName, operation.text);
        break;
      case "resolveConflict":
        await this.document.resolveDiskConflict(revision, operation.fileName, operation.diskHash, operation.resolution);
        break;
      case "undo":
        await this.document.undo(revision);
        break;
      case "redo":
        await this.document.redo(revision);
        break;
      case "save":
        await this.document.save(revision);
        break;
      case "query":
        break;
      case "refreshPlugins":
        await this.document.refreshPlugins(revision);
        break;
      case "verifyPlugin":
        await this.document.verifyPlugin(revision, operation.plugin);
        break;
      case "installPlugin":
        await this.document.installPlugin(revision, operation.packageName, operation.version);
        break;
      case "upgradePlugin":
        await this.document.upgradePlugin(revision, operation.packageName, operation.version);
        break;
      case "uninstallPlugin":
        await this.document.uninstallPlugin(revision, operation.packageName);
        break;
      case "repairPlugin":
        await this.document.repairPlugin(revision, operation.packageName);
        break;
    }
  }
  close(): void {
    this.closed = true;
  }
}
