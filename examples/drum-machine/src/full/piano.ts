import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { type PianoBank, type Project } from "@oxitone/core";
import assets from "./piano-assets.json" with { type: "json" };
import { outputRoot } from "./paths.js";

const cache = join(outputRoot, "piano");
const hash = (data: Uint8Array) => createHash("sha256").update(data).digest("hex");

/** Explicit preparation only. Normal source builds/tests do not download recordings. */
export async function preparePianos() {
  await mkdir(cache, { recursive: true });
  for (let index = 0; index < assets.files.length; index += 4) {
    await Promise.all(
      assets.files.slice(index, index + 4).map(async (file) => {
        const path = join(cache, file.file);
        if (existsSync(path) && hash(readFileSync(path)) === file.sha256) return;
        const response = await fetch(
          `https://raw.githubusercontent.com/sfzinstruments/SalamanderGrandPiano/${assets.commit}/Samples/${encodeURIComponent(file.file)}`,
          { signal: AbortSignal.timeout(60_000) },
        );
        if (!response.ok || !response.body) throw new Error(`Piano download failed: ${file.file} (${response.status})`);
        const reader = response.body.getReader(),
          chunks: Uint8Array[] = [];
        let length = 0;
        try {
          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            length += value.length;
            if (length > file.bytes) throw new Error(`Oversized piano recording: ${file.file}`);
            chunks.push(value);
          }
        } finally {
          await reader.cancel();
        }
        const data = Buffer.concat(chunks);
        if (length !== file.bytes || hash(data) !== file.sha256) throw new Error(`Piano hash mismatch: ${file.file}`);
        const partial = `${path}.${process.pid}.partial`;
        try {
          await writeFile(partial, data);
          await rename(partial, path);
        } finally {
          await rm(partial, { force: true });
        }
      }),
    );
  }
  const credit = `${assets.credit}\n${assets.source}\nPinned revision ${assets.commit}\nCC BY 3.0: https://creativecommons.org/licenses/by/3.0/\n`;
  await writeFile(join(cache, "CREDITS.txt"), credit);
  await writeFile(join(outputRoot, "CREDITS.txt"), credit);
}

/** 88-key library; register only the requested range to bound this project's PCM memory. */
export function pianoBank(project: Project, range: readonly [number, number] = [48, 84]): PianoBank {
  return {
    keyRange: range,
    layers: [2, 5, 9, 13].map((layer) =>
      assets.files
        .filter((file) => file.layer === layer && file.rootKey >= range[0] - 1 && file.rootKey <= range[1] + 1)
        .map((file) => ({
          rootKey: file.rootKey,
          gain: 1.2,
          sample: project.addSample({
            assetUri: join(cache, file.file),
            sha256: file.sha256,
            sampleRate: file.sampleRate,
            channels: file.channels as 1 | 2,
            frames: BigInt(file.frames),
            format: "flac",
          }),
        })),
    ),
  };
}
