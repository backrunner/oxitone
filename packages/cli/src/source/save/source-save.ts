import { realpath } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { SourceOwnership } from "../files/ownership.js";
import { checkSourceReads, type SourceRead } from "../files/read-set.js";
import { checkImage, clearJournal, prepareImages, publishImage, recoverJournal, writeJournal, type SourceSaveFile } from "./save-journal.js";
import { ordinaryDirectory, saveConflict, withSaveLock } from "./save-files.js";

export interface SourceSaveRequest {
  readonly files: readonly SourceSaveFile[];
  readonly reads: readonly SourceRead[];
  /** Called immediately before publication; a document can reject a superseded accepted revision. */
  readonly assertCurrent?: () => void;
}

/** Enrolled TS creation/replacement/deletion. Multiple publications are recoverable, not atomic. */
export class SourceSaveStore {
  private constructor(private readonly root: string, private readonly realRoot: string,
    private readonly directory: string, private readonly ownership: SourceOwnership) {}

  static async open(root: string, ownership: SourceOwnership): Promise<SourceSaveStore> {
    root = resolve(root);
    const realRoot = await ownership.assertRoot(root);
    const directory = join(realRoot, ".oxitone-source-save");
    await ordinaryDirectory(directory);
    const store = new SourceSaveStore(root, realRoot, directory, ownership);
    await store.recover(); return store;
  }
  private async checkRoot(): Promise<void> {
    if (await realpath(this.root) !== this.realRoot) saveConflict("source save root was relocated");
    await ordinaryDirectory(this.directory);
  }
  async recover(): Promise<boolean> {
    await this.checkRoot();
    return withSaveLock(this.directory, () => recoverJournal(this.directory, this.ownership));
  }
  async save(request: SourceSaveRequest): Promise<void> {
    // Copy the request before any asynchronous work; callers cannot mutate an in-flight write set.
    const files = request.files.map(file => ({ ...file }));
    const reads = request.reads.map(read => ({ ...read }));
    await this.checkRoot();
    await withSaveLock(this.directory, async () => {
      await recoverJournal(this.directory, this.ownership);
      try {
        const images = await prepareImages(files, this.ownership);
        await checkSourceReads(reads); request.assertCurrent?.();
        await writeJournal(this.directory, { version: 2, phase: "prepared", files: images });
        await checkSourceReads(reads); request.assertCurrent?.();
        for (const image of images) {
          await this.checkRoot();
          if (await checkImage(image, this.ownership) !== image.beforeHash) saveConflict("source changed before publication");
          request.assertCurrent?.();
          await publishImage(image, image.after, image.beforeHash);
        }
        // Read dependencies again, excluding the source files whose accepted postimages we just published.
        await checkSourceReads(reads.filter(read => !images.some(image => image.realPath === read.realPath)));
        for (const image of images) if (await checkImage(image, this.ownership) !== image.afterHash) saveConflict("source changed after publication");
        await writeJournal(this.directory, { version: 2, phase: "committed", files: images });
        await clearJournal(this.directory);
      } catch (error) {
        // A normal failure restores the old generation; a third image keeps the journal for conflict resolution.
        await recoverJournal(this.directory, this.ownership);
        throw error;
      }
    });
  }
}
