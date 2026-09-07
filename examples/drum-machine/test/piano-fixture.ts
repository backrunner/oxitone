import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import type { PianoBank, Project } from "@oxitone/core";

/** Small deterministic recorded-asset stand-in; no network or audio devices in CI. */
export function fixturePiano(directory: string) {
  return (project: Project): PianoBank => ({ keyRange: [21, 108],
    layers: Array.from({ length: 4 }, (_, layer) => {
      const frames = 48000, wav = Buffer.alloc(44 + frames * 2);
      wav.write("RIFF"); wav.writeUInt32LE(wav.length - 8, 4); wav.write("WAVEfmt ", 8);
      wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
      wav.writeUInt32LE(48000, 24); wav.writeUInt32LE(96000, 28); wav.writeUInt16LE(2, 32);
      wav.writeUInt16LE(16, 34); wav.write("data", 36); wav.writeUInt32LE(frames * 2, 40);
      for (let i = 0; i < frames; i++) wav.writeInt16LE(Math.round(32767 * 0.04 * 2 ** layer *
        Math.sin(2 * Math.PI * (220 + layer * 110) * i / 48000)), 44 + i * 2);
      const path = join(directory, `piano-layer-${layer}.wav`); writeFileSync(path, wav);
      return [{ rootKey: 60, sample: project.addSample({ assetUri: path, format: "wav",
        sha256: createHash("sha256").update(wav).digest("hex"), frames, channels: 1, sampleRate: 48000 }) }];
    }) });
}
