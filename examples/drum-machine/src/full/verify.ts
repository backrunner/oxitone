import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import type { Section } from "./shared.js";

/** Offline validation only: measure each arranged section of our PCM24 export. */
export function inspectSections(path: string, sections: readonly Section[], bpm: number, bars: number) {
  const bytes = readFileSync(path);
  assert.equal(bytes.toString("ascii", 0, 4), "RIFF");
  let dataOffset = 0, dataBytes = 0;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const kind = bytes.toString("ascii", offset, offset + 4), length = bytes.readUInt32LE(offset + 4);
    if (kind === "fmt ") {
      assert.equal(bytes.readUInt16LE(offset + 10), 2);
      assert.equal(bytes.readUInt32LE(offset + 12), 48000);
      assert.equal(bytes.readUInt16LE(offset + 22), 24);
    }
    if (kind === "data") { dataOffset = offset + 8; dataBytes = length; break; }
    offset += 8 + length + length % 2;
  }
  assert(dataOffset && dataOffset + dataBytes <= bytes.length);
  return sections.map(([name, start], index) => {
    const end = sections[index + 1]?.[1] ?? bars;
    const first = Math.round(start * 4 * 60 / bpm * 48000), last = Math.round(end * 4 * 60 / bpm * 48000);
    let energy = 0, peak = 0;
    for (let frame = first; frame < last; frame++) for (let ch = 0; ch < 2; ch++) {
      const value = bytes.readIntLE(dataOffset + frame * 6 + ch * 3, 3) / 8388608;
      energy += value * value; peak = Math.max(peak, Math.abs(value));
    }
    return { name, startBar: start + 1, bars: end - start,
      rmsDbfs: 10 * Math.log10(Math.max(1e-20, energy / ((last - first) * 2))),
      peakDbfs: 20 * Math.log10(Math.max(1e-10, peak)) };
  });
}
