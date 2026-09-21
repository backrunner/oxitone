import { randomUUID } from "node:crypto";
import { Vst3AutomationRecorder, type Vst3AutomationTake } from "@oxitone/core";
import {
  ErrorCode,
  OxitoneError,
  type DocumentView,
  type Vst3DocumentRecording,
  type Vst3WorkbenchCommand,
} from "@oxitone/protocol";
import type { Vst3Runtime } from "../../preview/vst3-client.js";
import type { EvaluatedConfigurationSite, ProjectEvaluation } from "../eval/project-evaluation.js";
import { resolveVst3Instance } from "../plugins/vst3-instance-target.js";
import { writeVst3Recording } from "../editing/vst3-recording-writer.js";
import { assertArrangement } from "../editing/arrangement-writer.js";

interface Owner {
  runtime: Vst3Runtime | undefined;
  ready(revision: number): ProjectEvaluation;
  isCurrent(revision: number, before: ProjectEvaluation): boolean;
  handle(site: { handle: string }): string;
  files(): ReadonlyMap<string, string>;
  emit(): void;
  transact(
    before: ProjectEvaluation,
    files: Map<string, string>,
    validate: (candidate: ProjectEvaluation) => void,
  ): Promise<DocumentView>;
}
type Endpoint = Awaited<ReturnType<typeof resolveVst3Instance>>;
interface Task {
  view: Vst3DocumentRecording;
  revision: number;
  before: ProjectEvaluation;
  site: EvaluatedConfigurationSite;
  cancelled: boolean;
  invalidated: boolean;
  setup?: Promise<void>;
  stop?: Promise<void>;
  endpoint?: Endpoint;
  recorder?: Vst3AutomationRecorder;
  take?: Vst3AutomationTake;
  timer?: ReturnType<typeof setInterval>;
}
type Start = Extract<Vst3WorkbenchCommand, { kind: "startRecording" }>;
const conflict = (message: string) => new OxitoneError(ErrorCode.PluginTaskConflict, message);
/** Owns one capture and its source acceptance; no native state or audio belongs to the UI. */
export class DocumentVst3Recording {
  private task: Task | undefined;
  private applying = false;
  constructor(private readonly owner: Owner) {}
  get view(): Vst3DocumentRecording | undefined {
    return this.task?.view;
  }
  private check(task: Task): void {
    if (this.task !== task || task.cancelled || task.invalidated || !this.owner.isCurrent(task.revision, task.before))
      throw conflict("Recording source or task changed; cancel this recording");
  }
  private failure(task: Task, error: unknown): void {
    task.view.status = task.take ? "captured" : "failed";
    task.view.error = {
      code: OxitoneError.isOxitoneError(error) ? error.code : ErrorCode.DraftInvalid,
      message: error instanceof Error ? error.message : String(error),
    };
    clearInterval(task.timer);
    this.owner.emit();
  }
  invalidate(): void {
    const task = this.task;
    if (!task || this.applying) return;
    task.invalidated = true;
    this.failure(task, conflict("Recording source changed; the old take cannot be applied"));
    void task.recorder?.cancel();
  }
  async start(revision: number, command: Start): Promise<void> {
    if (this.task) throw conflict("Finish or cancel the current recording first");
    const before = this.owner.ready(revision);
    const site = before.configurationSites.find((site) => this.owner.handle(site) === command.site);
    if (
      !site ||
      site.usages.length !== 1 ||
      (command.usage && (site.scope !== "reference" || site.usages[0]?.handle !== command.usage))
    )
      throw new OxitoneError(ErrorCode.EditScopeConflict, "Choose one isolated VST3 instance");
    const task: Task = {
      before,
      site,
      revision,
      cancelled: false,
      invalidated: false,
      view: {
        id: randomUUID(),
        instanceId: site.usages[0]!.handle,
        status: "starting",
        mode: command.mode,
        parameterIds: [...command.parameterIds],
      },
    };
    this.task = task;
    this.owner.emit();
    task.setup = this.setup(task);
    await task.setup;
  }
  private async setup(task: Task): Promise<void> {
    try {
      const endpoint = await resolveVst3Instance(
        task.before,
        task.site,
        String(task.revision + 1),
        this.owner.runtime,
        new AbortController().signal,
        () => this.check(task),
      );
      task.endpoint = endpoint;
      task.recorder = await Vst3AutomationRecorder.start(
        {
          controlVst3Instance: async (target, command, options) => {
            if (command.kind !== "discardEdits") this.check(task);
            // Do not abort an in-flight start: obtain its capture ID, then clean up that exact capture.
            return endpoint.runtime.control(
              endpoint.snapshotRevision,
              { instanceControlVersion: 1, ...target, command, ...options },
              new AbortController().signal,
            );
          },
        },
        endpoint.target,
        { mode: task.view.mode, parameterIds: task.view.parameterIds },
      );
      this.check(task);
      task.view.status = "recording";
      task.timer = setInterval(() => {
        if (task.recorder?.error) this.failure(task, task.recorder.error);
      }, 50);
      this.owner.emit();
    } catch (error) {
      await task.recorder?.cancel();
      if (this.task === task && !task.cancelled) this.failure(task, error);
      throw error;
    }
  }
  async stop(revision: number, id: string): Promise<void> {
    const task = this.require(id);
    if (revision !== task.revision) throw conflict("Recording revision changed");
    if (task.stop) return task.stop;
    task.stop = this.finish(task).finally(() => {
      delete task.stop;
    });
    await task.stop;
  }
  private async finish(task: Task): Promise<void> {
    try {
      await task.setup;
      this.check(task);
      clearInterval(task.timer);
      task.view.status = "stopping";
      delete task.view.error;
      this.owner.emit();
      task.take ??= await task.recorder!.stop();
      this.check(task);
      await task.endpoint!.current();
      const inventory = await task.endpoint!.runtime.inventory(
        task.endpoint!.snapshotRevision,
        new AbortController().signal,
      );
      if (inventory.state !== "active" || inventory.graphGeneration !== task.take.target.graphGeneration)
        throw conflict("Recording graph was replaced before acceptance");
      this.check(task);
      if (task.take.spans.length) {
        const candidate = writeVst3Recording(task.before, this.owner.files(), task.site, task.take);
        this.check(task);
        this.applying = true;
        try {
          await this.owner.transact(task.before, candidate.files, (evaluated) => {
            if (task.cancelled || this.task !== task) throw conflict("Recording acceptance was cancelled");
            assertArrangement(task.before.frame.snapshot, candidate.expected, evaluated.frame.snapshot);
          });
        } finally {
          this.applying = false;
        }
      }
      if (this.task === task) this.task = undefined;
      this.owner.emit();
    } catch (error) {
      if (this.task === task && !task.cancelled) this.failure(task, error);
      throw error;
    }
  }
  private require(id: string): Task {
    if (!this.task || this.task.view.id !== id) throw conflict("Recording is no longer current");
    return this.task;
  }
  async cancel(id: string): Promise<void> {
    const task = this.require(id);
    task.cancelled = true;
    clearInterval(task.timer);
    await task.setup?.catch(() => {});
    await task.recorder?.cancel();
    // An in-flight transaction checks cancellation immediately before accepting its candidate.
    await task.stop?.catch(() => {});
    if (this.task === task) this.task = undefined;
    this.owner.emit();
  }
  close(): void {
    const task = this.task;
    if (task) void this.cancel(task.view.id);
  }
}
