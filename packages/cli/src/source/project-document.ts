import { randomUUID } from "node:crypto";
import { writeArrangement, assertArrangement } from "./arrangement-writer.js";
import { writeProjectEdit } from "./project-edit-writer.js";
import { preparePluginAssignment, type PluginAssignment } from "./plugin-assignment.js";
import { dirname, resolve } from "node:path";
import { Pattern } from "@oxitone/core";
import { canonicalEncode, ErrorCode, OxitoneError, type DocumentView, type NoteEdit, type PreviewSnapshotFrame, type MaterializationReview, type AutomationRangeEdit } from "@oxitone/protocol";
import { projectSourceFiles } from "./project-files.js";
import { evaluateSourceProject, type ProjectEvaluation } from "./project-evaluation.js";
import { assertProjectNoteEdit, assertProjectAutomationEdit, assertFrameConfiguration } from "./project-equivalence.js";
import { validateProjectFrame } from "./project-validate.js";
import { writePatternEdit, type PatternWriteResult } from "./pattern-writer.js";
import { noteEditExtent } from "./pattern-extent.js";
import { materializePatternReference, writeLiteralPatternEdit } from "./materialize.js";
import { checkSourceReads } from "./read-set.js";
import { sourceHash } from "./syntax.js";
import { SourceSaveStore } from "./source-save.js";
import { SourceOwnership } from "./ownership.js";
import { checkConflict, checkProjectText, projectModified, projectSaveFiles, readProjectDisk } from "./project-disk.js";
import { imageHash } from "./save-journal.js";
import { ProjectPluginCatalog } from "./project-plugins.js";
import { writeAutomationRange } from "./automation-writer.js";
import { writeConfigurationEdit } from "./configuration-writer.js";
import { assertProjectConfigurationEdit, assertProjectEquivalent } from "./project-equivalence.js";
import type { ConfigurationEdit } from "@oxitone/protocol";
import type { RackMaterializationReview } from "@oxitone/protocol";
import { materializeRack } from "./rack-writer.js";
import { writeEffectOrder, projectOrderEdit } from "./effect-order-writer.js";
import { projectSites } from "./project-sites.js";
import { PluginLifecycle, type PluginTaskRunner } from "./plugin-lifecycle.js";
import { mergeThreeWay } from "./three-way-merge.js";
import { sourceSpan } from "./source-timing.js";

export interface ProjectDocumentOptions { entry: string; projectRoot?: string; sourceRoots?: readonly string[]; readPaths?: readonly string[]; pluginTaskRunner?: PluginTaskRunner }
function changed(): never { throw new OxitoneError(ErrorCode.SourceChanged, "document revision changed or conflicts with external source"); }
const textBytes = (files: ReadonlyMap<string, string>) => [...files.values()].reduce((sum, text) => sum + Buffer.byteLength(text), 0);
const diagnostic = (error: unknown) => ({ code: OxitoneError.isOxitoneError(error) ? error.code : ErrorCode.DraftInvalid,
  message: error instanceof Error ? error.message : String(error) });

/** One document owns all local code buffers, accepted project data and the shared Undo/Save history. */
export class ProjectDocument {
  readonly sessionId = randomUUID();
  private files: Map<string, string>;
  private disk: Map<string, string>;
  private revision = 0;
  private acceptedRevision = -1;
  private savedRevision = 0;
  private status: DocumentView["status"] = "building";
  private saving = false;
  private conflicts: DocumentView["conflicts"] = [];
  private materialization: { review: MaterializationReview; files: Map<string, string>; evaluated: ProjectEvaluation } | undefined;
  private rackMaterialization: { review: RackMaterializationReview; files: Map<string, string>; evaluated: ProjectEvaluation } | undefined;
  private error?: DocumentView["diagnostic"];
  private accepted?: ProjectEvaluation;
  private generation = 0;
  private controller = new AbortController();
  private readonly listeners = new Set<(view: DocumentView) => void>();
  private readonly history: Map<string, string>[] = [];
  private readonly future: Map<string, string>[] = [];
  private readonly plugins: ProjectPluginCatalog;
  private readonly pluginLifecycle: PluginLifecycle;
  private constructor(private readonly options: ProjectDocumentOptions, private readonly ownership: SourceOwnership,
    private readonly store: SourceSaveStore, files: Map<string, string>) {
    this.files = files; this.disk = new Map(files); this.plugins = new ProjectPluginCatalog(options.projectRoot ?? dirname(options.entry));
    this.pluginLifecycle = new PluginLifecycle(options.projectRoot ?? dirname(options.entry), options.pluginTaskRunner);
  }

  static async open(options: ProjectDocumentOptions): Promise<ProjectDocument> {
    const entry = resolve(options.entry), root = resolve(options.projectRoot ?? dirname(entry));
    const roots = options.sourceRoots ?? [root];
    // Recover before discovery: a prepared journal can remove or restore whole source files.
    await SourceSaveStore.open(root, await SourceOwnership.open(roots, []));
    const { ownership, files } = await projectSourceFiles(roots);
    await ownership.assertWritable(entry);
    const store = await SourceSaveStore.open(root, ownership);
    const document = new ProjectDocument({ ...options, entry, projectRoot: root }, ownership, store, files);
    await document.rebuild();
    if (document.status !== "ready") await document.refreshPlugins(document.revision);
    return document;
  }
  get view(): DocumentView {
    return structuredClone({ sessionId: this.sessionId, projectRoot: this.options.projectRoot!, revision: this.revision, acceptedRevision: this.acceptedRevision,
      savedRevision: this.savedRevision, status: this.status, saving: this.saving,
      modified: projectModified(this.files, this.disk), ...projectSites(this.accepted, site => this.handle(site)),
      files: [...this.files].map(([path, text]) => ({ path, text })), conflicts: this.conflicts,
      plugins: this.plugins.view(this.accepted?.frame),
      ...(this.rackMaterialization ? { rackMaterialization: this.rackMaterialization.review } : {}),
      ...(this.materialization ? { materialization: this.materialization.review } : {}), ...(this.error ? { diagnostic: this.error } : {}) });
  }
  get frame(): PreviewSnapshotFrame | undefined {
    if (!this.accepted) return;
    const frame = structuredClone(this.accepted.frame);
    frame.snapshot.revision = String(this.acceptedRevision + 1);
    frame.hash = sourceHash(canonicalEncode(JSON.parse(JSON.stringify(frame)))); return frame;
  }
  get dependencyPaths(): readonly string[] { return [...new Set([...this.files.keys(), ...this.disk.keys(), ...this.plugins.dependencyPaths, ...(this.accepted?.reads ?? []).flatMap(read => [read.path, read.realPath])])]; }
  subscribe(listener: (view: DocumentView) => void): () => void {
    this.listeners.add(listener); this.notify(listener); return () => { this.listeners.delete(listener); };
  }
  private notify(listener: (view: DocumentView) => void): void { try { listener(this.view); } catch { this.listeners.delete(listener); } }
  private emit(): void { for (const listener of this.listeners) this.notify(listener); }
  private handle(site: { handle: string }): string { return `${this.sessionId}:${this.acceptedRevision}:${site.handle}`; }
  private current(revision: number): void { if (this.status === "closed" || this.saving || this.revision !== revision) changed(); }
  private ready(revision: number): ProjectEvaluation {
    this.current(revision);
    if (this.status !== "ready" || !this.accepted || this.acceptedRevision !== revision) throw new OxitoneError(ErrorCode.DraftInvalid, "editing requires a valid, conflict-free project");
    return this.accepted;
  }
  private begin(): number { this.materialization = undefined; this.rackMaterialization = undefined; this.controller.abort(); this.controller = new AbortController(); return ++this.generation; }
  private check(generation: number): void { if (this.status === "closed" || generation !== this.generation) changed(); }
  private record(stack: Map<string, string>[], files: Map<string, string>): void {
    stack.push(new Map(files));
    while (stack.length > 128 || stack.reduce((sum, files) => sum + textBytes(files), 0) > 32 * 1024 * 1024) stack.shift();
  }
  private async evaluate(files: ReadonlyMap<string, string>, generation: number): Promise<ProjectEvaluation> {
    checkProjectText(files);
    const removed = [...this.disk.keys()].filter(path => !files.has(path));
    const result = await evaluateSourceProject(this.options.entry, files, this.ownership, this.controller.signal, this.options.readPaths, removed);
    this.check(generation);
    validateProjectFrame(result.frame);
    await checkSourceReads(result.reads); this.check(generation); return result;
  }
  private async rebuild(): Promise<DocumentView> {
    if (this.conflicts.length) { this.begin(); this.status = "conflict"; this.emit(); return this.view; }
    const generation = this.begin(); this.status = "building"; this.error = undefined; this.emit();
    try {
      this.accepted = await this.evaluate(this.files, generation); this.acceptedRevision = this.revision; this.status = "ready";
      await this.refreshPlugins(this.revision);
    }
    catch (error) { this.check(generation); this.status = "invalid"; this.error = diagnostic(error); }
    this.emit(); return this.view;
  }
  async changeCode(revision: number, fileName: string, text: string): Promise<DocumentView> {
    this.current(revision); fileName = resolve(fileName);
    if (!this.files.has(fileName)) throw new OxitoneError(ErrorCode.EditNotRepresentable, "file is not part of the source document");
    if (this.files.get(fileName) === text) return this.view;
    const files = new Map(this.files); files.set(fileName, text); checkProjectText(files);
    this.record(this.history, this.files); this.future.length = 0;
    this.files = files; this.revision++; return this.rebuild();
  }
  async createFile(revision: number, fileName: string, text: string): Promise<DocumentView> {
    this.current(revision); const generation = this.generation;
    fileName = resolve(this.options.projectRoot!, fileName);
    if (this.files.has(fileName)) throw new OxitoneError(ErrorCode.EditTargetMissing, "source file already exists in the document");
    const files = new Map(this.files); files.set(fileName, text); checkProjectText(files);
    if (this.disk.has(fileName)) {
      await this.ownership.assertWritable(fileName); this.check(generation); this.current(revision);
    } else await this.ownership.enroll(fileName, () => { this.check(generation); this.current(revision); });
    this.record(this.history, this.files); this.future.length = 0;
    this.files = files; this.revision++; return this.rebuild();
  }
  async edit(revision: number, handle: string, operations: readonly NoteEdit[], placement?: string): Promise<DocumentView> {
    return this.noteCandidate(revision, handle, operations, "edit", placement);
  }
  async planMaterialize(revision: number, handle: string, operations: readonly NoteEdit[] = [], placement?: string): Promise<DocumentView> {
    return this.noteCandidate(revision, handle, operations, "materialize", placement);
  }
  private async noteCandidate(revision: number, handle: string, operations: readonly NoteEdit[], mode: "edit" | "materialize", placement?: string): Promise<DocumentView> {
    const before = this.ready(revision);
    const site = before.sites.find(site => this.handle(site) === handle && site.invocations === 1);
    if (!site) throw new OxitoneError(ErrorCode.EditTargetMissing, "source definition no longer exists");
    if (placement && (site.scope !== "reference" || site.placements.length !== 1 || site.placements[0] !== placement)) {
      throw new OxitoneError(ErrorCode.EditScopeConflict, "this source boundary does not isolate the selected placement");
    }
    if (!operations.length && mode === "edit") return this.view;
    const lengthBeats = noteEditExtent(site.source, operations);
    const request = { fileName: site.fileName, text: this.files.get(site.fileName)!, anchor: site.anchor, source: site.source, operations,
      ...(lengthBeats === undefined ? {} : { lengthBeats }) };
    const detached = mode === "materialize" ? materializePatternReference(request) : undefined;
    const candidate: PatternWriteResult = detached ?? writeLiteralPatternEdit(request) ?? writePatternEdit(request);
    if ([...Pattern.fromSource(site.source).notes, ...Pattern.fromSource(candidate.source).notes].some(note => note.chance !== undefined && note.chance !== 1)) {
      throw new OxitoneError(ErrorCode.EditNotRepresentable, "probabilistic note editing requires the musical-origin runtime migration");
    }
    if (before.frame.snapshot.patternClips.some(clip => (clip.patternId === site.patternId || before.frame.snapshot.patterns.find(p => p.id === clip.patternId)?.parts?.some(part => part.patternId === site.patternId)) && (!placement || clip.id === placement) && clip.probability !== undefined && clip.probability !== 1)) {
      throw new OxitoneError(ErrorCode.EditNotRepresentable, "probabilistic placement editing requires the musical-origin runtime migration");
    }
    const files = new Map(this.files); files.set(site.fileName, candidate.text);
    const generation = this.begin(); this.status = "building"; this.emit();
    try {
      await checkSourceReads(before.reads); this.check(generation);
      const evaluated = await this.evaluate(files, generation);
      assertProjectNoteEdit(before.frame.snapshot, evaluated.frame.snapshot, site.patternId, candidate.source, placement);
      // Plugin registrations and UI/resource configuration are also outside the scope of a note edit.
      assertFrameConfiguration(before.frame, evaluated.frame);
      await checkSourceReads(before.reads); this.check(generation);
      if (detached) {
        this.materialization = { files, evaluated, review: { planId: randomUUID(), baseRevision: revision, fileName: site.fileName,
          beforeText: request.text, afterText: candidate.text, affectedClips: placement ? [placement] : before.frame.snapshot.patternClips.filter(clip => clip.patternId === site.patternId || before.frame.snapshot.patterns.find(p => p.id === clip.patternId)?.parts?.some(part => part.patternId === site.patternId)).map(clip => clip.id),
          beforeNotes: detached.summary.beforeNotes, afterNotes: detached.summary.afterNotes, losesGeneratorLink: true, retainsDependencyImports: true,
          retainsOriginalEvaluation: detached.summary.retainsOriginalEvaluation } };
        this.status = "ready"; this.error = undefined;
      } else this.adopt(files, evaluated);
    } catch (error) { this.check(generation); this.status = "ready"; this.error = diagnostic(error); this.emit(); throw error; }
    this.emit(); return this.view;
  }
  private adopt(files: Map<string, string>, evaluated: ProjectEvaluation): void {
    this.record(this.history, this.files); this.future.length = 0; this.materialization = undefined;
    this.rackMaterialization = undefined;
    this.files = files; this.accepted = evaluated; this.acceptedRevision = ++this.revision; this.status = "ready"; this.error = undefined;
  }
  private async transact(before: ProjectEvaluation, files: Map<string, string>, validate: (candidate: ProjectEvaluation) => void,
    accept: (candidate: ProjectEvaluation) => void = candidate => this.adopt(files, candidate), expectedFrame = before.frame): Promise<DocumentView> {
    const generation = this.begin(); this.status = "building"; this.emit();
    try {
      await checkSourceReads(before.reads); this.check(generation);
      const evaluated = await this.evaluate(files, generation); validate(evaluated);
      assertFrameConfiguration(expectedFrame, evaluated.frame);
      await checkSourceReads(before.reads); this.check(generation);
      accept(evaluated); this.status = "ready"; this.error = undefined;
    } catch (error) { this.check(generation); this.status = "ready"; this.error = diagnostic(error); this.emit(); throw error; }
    this.emit(); return this.view;
  }
  async editAutomationRange(revision: number, handle: string, edit: AutomationRangeEdit, lane?: string, clipId?: string): Promise<DocumentView> {
    const before = this.ready(revision), site = before.automationSites.find(site => this.handle(site) === handle);
    if (!site) throw new OxitoneError(ErrorCode.EditTargetMissing, "automation source is no longer present");
    if (lane && (site.scope !== "reference" || site.lanes.length !== 1 || site.lanes[0] !== lane)) {
      throw new OxitoneError(ErrorCode.EditScopeConflict, "source boundary does not isolate this lane");
    }
    let sourceEdit = edit;
    if (clipId !== undefined) {
      const clip = before.frame.snapshot.automationClips?.find(item => item.id === clipId);
      if (!clip || !site.lanes.includes(clip.laneId)) throw new OxitoneError(ErrorCode.EditTargetMissing, "automation clip is not part of this source site");
      const siblings = (before.frame.snapshot.automationClips ?? []).filter(item => item.laneId === clip.laneId && item.enabled !== false);
      if (siblings.length !== 1) throw new OxitoneError(ErrorCode.EditScopeConflict, "timeline automation editing requires a lane used by exactly one enabled clip");
      const start = Number(clip.startBeat.numerator) / Number(clip.startBeat.denominator);
      const duration = clip.durationBeats === undefined ? 0 : Number(clip.durationBeats.numerator) / Number(clip.durationBeats.denominator);
      if (edit.start < start || edit.end > start + duration) throw new OxitoneError(ErrorCode.EditScopeConflict, "automation edit falls outside the selected clip");
      // Replacement points are already local to the edited range (the same
      // convention as source-local replaceRange); only the clip window moves.
      sourceEdit = { ...edit, start: edit.start - start, end: edit.end - start };
    }
    const candidate = writeAutomationRange(site.fileName, this.files.get(site.fileName)!, site, sourceEdit);
    const files = new Map(this.files); files.set(site.fileName, candidate.text);
    return this.transact(before, files, evaluated => assertProjectAutomationEdit(before.frame.snapshot, evaluated.frame.snapshot, lane ? [lane] : site.lanes, candidate.source));
  }
  async arrange(revision: number, edit: import("@oxitone/protocol").ArrangementEdit): Promise<DocumentView> {
    const before = this.ready(revision), candidate = writeArrangement(before, this.files, edit);
    if (!candidate) return this.view;
    return this.transact(before, candidate.files, evaluated => assertArrangement(before.frame.snapshot, candidate.expected, evaluated.frame.snapshot));
  }
  async configure(revision: number, edit: import("@oxitone/protocol").ProjectEdit): Promise<DocumentView> {
    const before = this.ready(revision), candidate = writeProjectEdit(before, this.files, edit);
    if (!candidate) return this.view;
    return this.transact(before, candidate.files, evaluated => assertArrangement(before.frame.snapshot, candidate.expected, evaluated.frame.snapshot));
  }
  async assignPlugin(revision: number, input: PluginAssignment): Promise<DocumentView> {
    const before = this.ready(revision), generation = this.generation;
    await this.plugins.verify(input.plugin, before.frame, this.controller.signal, () => { this.check(generation); this.ready(revision); });
    const candidate = preparePluginAssignment(before, this.files, this.plugins, input);
    return this.transact(candidate.before, candidate.files, evaluated => assertArrangement(before.frame.snapshot, candidate.expected, evaluated.frame.snapshot), undefined, candidate.expectedFrame);
  }
  async confirmMaterialize(revision: number, planId: string): Promise<DocumentView> {
    const before = this.ready(revision), pending = this.materialization ?? this.rackMaterialization, generation = this.generation;
    if (!pending || pending.review.planId !== planId || pending.review.baseRevision !== revision) changed();
    await checkSourceReads(before.reads); await checkSourceReads(pending.evaluated.reads);
    this.check(generation); this.ready(revision);
    if (this.materialization !== pending && this.rackMaterialization !== pending) changed();
    this.begin(); this.adopt(pending.files, pending.evaluated); this.emit(); return this.view;
  }
  async editConfiguration(revision: number, handle: string, edit: ConfigurationEdit, usage?: string): Promise<DocumentView> {
    const before = this.ready(revision), site = before.configurationSites.find(site => this.handle(site) === handle);
    if (!site) throw new OxitoneError(ErrorCode.EditTargetMissing, "plugin configuration is no longer present");
    if (usage && (site.scope !== "reference" || site.usages.length !== 1 || site.usages[0]!.handle !== usage)) {
      throw new OxitoneError(ErrorCode.EditScopeConflict, "source boundary does not isolate this plugin use");
    }
    const candidate = writeConfigurationEdit(site.fileName, this.files.get(site.fileName)!, site, edit);
    if (canonicalEncode(candidate.config) === canonicalEncode(site.config)) return this.view;
    const files = new Map(this.files); files.set(site.fileName, candidate.text);
    return this.transact(before, files, evaluated => assertProjectConfigurationEdit(before.frame.snapshot, evaluated.frame.snapshot, site.usages, candidate.config));
  }
  async editEffectOrder(revision: number, handle: string, owner: string, order: readonly string[]): Promise<DocumentView> {
    const done = sourceSpan("effect-order");
    try {
      const before = this.ready(revision), site = before.effectOwnerSites.find(site => this.handle(site) === handle && site.owner === owner);
      if (!site) throw new OxitoneError(ErrorCode.EditTargetMissing, "isolated effect owner boundary no longer exists");
      const projectEdit = projectOrderEdit(before, owner, order);
      if (projectEdit) return await this.configure(revision, projectEdit);
      const text = this.files.get(site.fileName)!;
      const candidate = writeEffectOrder(text, site, before.frame.snapshot, order);
      if (candidate.text === text) return this.view;
      const files = new Map(this.files); files.set(site.fileName, candidate.text);
      return await this.transact(before, files, evaluated => assertProjectEquivalent(candidate.expected, evaluated.frame.snapshot));
    } finally { done(); }
  }
  cancelMaterialize(revision: number, planId: string): DocumentView {
    this.current(revision); if (this.materialization?.review.planId === planId) { this.materialization = undefined; this.emit(); }
    if (this.rackMaterialization?.review.planId === planId) { this.rackMaterialization = undefined; this.emit(); }
    return this.view;
  }
  async planMaterializeRack(revision: number, handle: string, owner?: string): Promise<DocumentView> {
    const before = this.ready(revision), site = before.rackSites.find(site => this.handle(site) === handle);
    if (!site) throw new OxitoneError(ErrorCode.EditTargetMissing, "effect chain is no longer present");
    if (owner && (site.scope !== "reference" || site.usages.length !== 1 || site.usages[0]!.owner !== owner)) {
      throw new OxitoneError(ErrorCode.EditScopeConflict, "source boundary does not isolate this chain");
    }
    const text = this.files.get(site.fileName)!, candidate = materializeRack(site.fileName, text, site);
    const files = new Map(this.files); files.set(site.fileName, candidate.text);
    return this.transact(before, files, evaluated => assertProjectEquivalent(before.frame.snapshot, evaluated.frame.snapshot), evaluated => {
      this.rackMaterialization = { files, evaluated, review: { planId: randomUUID(), baseRevision: revision, fileName: site.fileName,
        beforeText: text, afterText: candidate.text, affectedOwners: site.usages.map(usage => usage.owner), effects: site.effects.length,
        retainsOriginalEvaluation: candidate.retainsOriginalEvaluation, retainsDependencyImports: true, losesGeneratorLink: true } };
    });
  }
  async undo(revision: number): Promise<DocumentView> {
    this.current(revision); const files = this.history.pop(); if (!files) return this.view;
    this.record(this.future, this.files); this.files = files; this.revision++; return this.rebuild();
  }
  async redo(revision: number): Promise<DocumentView> {
    this.current(revision); const files = this.future.pop(); if (!files) return this.view;
    this.record(this.history, this.files); this.files = files; this.revision++; return this.rebuild();
  }
  async save(revision: number): Promise<DocumentView> {
    const accepted = this.ready(revision);
    const files = projectSaveFiles(this.files, this.disk);
    const generation = this.begin();
    if (!files.length) { this.emit(); return this.view; }
    this.materialization = undefined; this.saving = true; this.emit();
    try {
      await this.store.save({ files, reads: accepted.reads, assertCurrent: () => this.check(generation) });
      this.disk = new Map(this.files); this.savedRevision = revision;
      for (const file of files) {
        const owned = await this.ownership.assertWritable(file.path);
        accepted.reads = accepted.reads.map(read => read.realPath === owned.realPath ? { ...read, sha256: imageHash(file.text) } : read);
      }
    } finally { this.saving = false; this.emit(); }
    return this.view;
  }
  async synchronizeDisk(): Promise<DocumentView> {
    this.current(this.revision); if (this.status === "building") return this.view;
    const generation = this.generation, revision = this.revision;
    const check = () => { this.check(generation); this.current(revision); };
    const external = await readProjectDisk(this.files, this.disk, check); check();
    if (external.conflicts.length) {
      if (JSON.stringify(this.conflicts) !== JSON.stringify(external.conflicts)) { this.begin(); this.revision++; }
      this.conflicts = external.conflicts; this.status = "conflict";
      this.error = { code: ErrorCode.SourceChanged, message: "external source conflicts with the unsaved draft" };
      this.emit(); return this.view;
    }
    if (!external.changed && this.status !== "invalid" && this.status !== "conflict") {
      try {
        if (this.accepted) await checkSourceReads(this.accepted.reads); check();
        try { await this.plugins.checkReads(); check(); } catch { check(); return this.refreshPlugins(revision); }
        return this.view;
      }
      catch { check(); }
    }
    this.conflicts = []; this.files = external.files; this.disk = external.disk;
    this.history.length = 0; this.future.length = 0; this.revision++;
    if (!projectModified(this.files, this.disk)) this.savedRevision = this.revision;
    return this.rebuild();
  }
  async resolveDiskConflict(revision: number, fileName: string, diskHash: string, resolution: "use-disk" | "keep-draft" | "merge"): Promise<DocumentView> {
    this.current(revision); const generation = this.generation;
    const conflict = this.conflicts.find(item => item.path === resolve(fileName));
    await checkConflict(conflict, diskHash); this.check(generation); this.current(revision);
    const draft = this.files.get(conflict!.path);
    const merged = resolution === "merge" && draft !== undefined ? mergeThreeWay(conflict!.baseline, draft, conflict!.disk) : undefined;
    if (resolution === "merge" && merged === undefined) throw new OxitoneError(ErrorCode.EditScopeConflict, "draft and disk edits overlap; choose one side or edit the conflict manually");
    this.disk.set(conflict!.path, conflict!.disk);
    if (resolution === "use-disk") this.files.set(conflict!.path, conflict!.disk);
    else if (resolution === "merge") this.files.set(conflict!.path, merged!);
    this.conflicts = this.conflicts.filter(item => item !== conflict);
    this.history.length = 0; this.future.length = 0; this.revision++;
    if (!projectModified(this.files, this.disk)) this.savedRevision = this.revision;
    return this.rebuild();
  }
  async refreshPlugins(revision: number): Promise<DocumentView> {
    this.current(revision); const generation = this.generation;
    try { await this.plugins.refresh(this.accepted?.frame, () => { this.check(generation); this.current(revision); }); }
    catch (error) { this.check(generation); this.error = diagnostic(error); }
    this.emit(); return this.view;
  }
  async verifyPlugin(revision: number, handle: string): Promise<DocumentView> {
    this.current(revision); const generation = this.generation;
    try { await this.plugins.verify(handle, this.accepted?.frame, this.controller.signal, () => { this.check(generation); this.current(revision); }); }
    finally { this.emit(); }
    return this.view;
  }
  private async pluginTask(revision: number, task: import("./plugin-lifecycle.js").PluginTask): Promise<DocumentView> {
    this.current(revision);
    if (!["ready", "invalid"].includes(this.status) || this.saving || this.conflicts.length) throw new OxitoneError(ErrorCode.PluginTaskConflict, "plugin tasks require an idle project without pending Save or conflicts");
    const previousStatus = this.status;
    const generation = this.begin(); this.status = "building"; this.error = undefined; this.emit();
    try {
      await this.pluginLifecycle.run(task, this.controller.signal); this.check(generation);
      return await this.rebuild();
    } catch (error) {
      this.check(generation); this.status = previousStatus; this.error = diagnostic(error); this.emit(); throw error;
    }
  }
  installPlugin(revision: number, packageName: string, version?: string): Promise<DocumentView> {
    return this.pluginTask(revision, { kind: "install", packageName, ...(version === undefined ? {} : { version }) });
  }
  upgradePlugin(revision: number, packageName: string, version?: string): Promise<DocumentView> {
    return this.pluginTask(revision, { kind: "upgrade", packageName, ...(version === undefined ? {} : { version }) });
  }
  uninstallPlugin(revision: number, packageName: string): Promise<DocumentView> {
    return this.pluginTask(revision, { kind: "uninstall", packageName });
  }
  repairPlugin(revision: number, packageName: string): Promise<DocumentView> {
    return this.pluginTask(revision, { kind: "repair", packageName });
  }
  close(): void { this.begin(); this.status = "closed"; this.emit(); this.listeners.clear(); }
}
