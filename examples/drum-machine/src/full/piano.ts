import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { mkdir, writeFile, rename } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { importSample } from "oxitone";
import { multisampler, type Project, type SampleRegion } from "@oxitone/core";
import assets from "./piano-assets.json" with { type: "json" };

// Works from both src/full and dist/full. Only the explicit prepare command downloads assets.
export const outputRoot = fileURLToPath(new URL("../../../../target/examples/full-songs/", import.meta.url));
export const pianoCache = join(outputRoot, "piano");
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");

export async function preparePiano(): Promise<void> {
  await mkdir(pianoCache, { recursive: true });
  for (let start = 0; start < assets.files.length; start += 3) {
    await Promise.all(assets.files.slice(start, start + 3).map(async file => {
      const path = join(pianoCache, file.file);
      if (existsSync(path) && hash(readFileSync(path)) === file.sha256) return;
      const url = `https://raw.githubusercontent.com/sgossner/VSCO-2-CE/${assets.commit}/${file.path.split("/").map(encodeURIComponent).join("/")}`;
      const response = await fetch(url, { signal: AbortSignal.timeout(60_000) });
      if (!response.ok) throw new Error(`Piano download failed: ${response.status} ${file.file}`);
      const reader = response.body!.getReader();
      const chunks: Uint8Array[] = []; let bytes = 0;
      try {
        while (true) {
          const { done, value } = await reader.read(); if (done) break;
          bytes += value.length;
          if (bytes > file.bytes) throw new Error(`Oversized piano asset: ${file.file}`);
          chunks.push(value);
        }
      } finally { await reader.cancel(); }
      const data = Buffer.concat(chunks);
      if (bytes !== file.bytes || hash(data) !== file.sha256) throw new Error(`Piano hash mismatch: ${file.file}`);
      await writeFile(`${path}.partial`, data); await rename(`${path}.partial`, path);
    }));
  }
  const samples = assets.files.map(file => {
    const sample = importSample(join(pianoCache, file.file));
    if (sample.sha256 !== file.sha256) throw new Error(`Piano asset changed: ${file.file}`);
    return { ...file, sample: { ...sample, frames: sample.frames.toString() } };
  });
  await writeFile(join(pianoCache, "bank.json.partial"), JSON.stringify(samples));
  await rename(join(pianoCache, "bank.json.partial"), join(pianoCache, "bank.json"));
  await writeFile(join(pianoCache, "CREDITS.txt"), `${assets.credit}\n${assets.source}\nPinned commit ${assets.commit}\nCC0-1.0: https://creativecommons.org/publicdomain/zero/1.0/\n`);
}

/** One native instrument / one piano-roll track, with 13 key zones × 3 recorded dynamics. */
export function uprightPiano(project: Project) {
  const path = join(pianoCache, "bank.json");
  if (!existsSync(path)) throw new Error("Piano assets are missing. Run pnpm example:songs:prepare first.");
  const files = JSON.parse(readFileSync(path, "utf8")) as (typeof assets.files[number] & {
    sample: Omit<ReturnType<typeof importSample>, "frames"> & { frames: string }
  })[];
  const regions: SampleRegion[] = files.map(file => ({
    sample: project.addSample({ ...file.sample, assetUri: join(pianoCache, file.file), frames: BigInt(file.sample.frames) }),
    // Upstream MappingChart.txt: 010=41, 012=45, …, 034=89.
    // At most two semitones of repitch; deliberately silent outside MIDI 39…91.
    rootKey: file.rootKey, keyRange: [file.rootKey - 2, file.rootKey === 89 ? 91 : file.rootKey + 1],
    velocityRange: file.layer === 1 ? [1, 50] : file.layer === 2 ? [51, 88] : [89, 127],
    gain: 2,
  }));
  return multisampler(regions, { velocitySensitivity: 0.35, amp: { attack: 0.001, decay: 0, sustain: 1, release: 0.24 } });
}
