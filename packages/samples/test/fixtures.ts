import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach } from "vitest";

const directories: string[] = [];
afterEach(() => {
  for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

export function tempDirectory(): string {
  const directory = mkdtempSync(join(tmpdir(), "oxitone-import-"));
  directories.push(directory);
  return directory;
}

/** Deterministic PCM16 fixture; no external codec or binary asset required. */
export function wavFile(directory: string, channels = 1, rate = 48_000): { path: string; bytes: Buffer } {
  const frames = rate / 10;
  const bytes = Buffer.alloc(44 + frames * channels * 2);
  bytes.write("RIFF"); bytes.writeUInt32LE(bytes.length - 8, 4); bytes.write("WAVEfmt ", 8);
  bytes.writeUInt32LE(16, 16); bytes.writeUInt16LE(1, 20); bytes.writeUInt16LE(channels, 22);
  bytes.writeUInt32LE(rate, 24); bytes.writeUInt32LE(rate * channels * 2, 28);
  bytes.writeUInt16LE(channels * 2, 32); bytes.writeUInt16LE(16, 34);
  bytes.write("data", 36); bytes.writeUInt32LE(bytes.length - 44, 40);
  for (let frame = 0; frame < frames; frame++) {
    const sample = Math.round(0.5 * 32767 * Math.sin(2 * Math.PI * 440 * frame / rate));
    for (let channel = 0; channel < channels; channel++) bytes.writeInt16LE(sample, 44 + (frame * channels + channel) * 2);
  }
  const path = join(directory, "音频 loop.dat");
  writeFileSync(path, bytes);
  return { path, bytes };
}

export function wavSamples(path: string): number[] {
  const bytes = readFileSync(path);
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const length = bytes.readUInt32LE(offset + 4);
    if (bytes.toString("ascii", offset, offset + 4) === "data") {
      return Array.from({ length: length / 4 }, (_, index) => bytes.readFloatLE(offset + 8 + index * 4));
    }
    offset += 8 + length + (length % 2);
  }
  throw new Error("WAV missing data chunk");
}
