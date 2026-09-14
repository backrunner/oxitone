import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import type { Section } from "./shared.js";

/** Offline validation only: measure each arranged section of our PCM24 export. */
export function inspectSections(path: string, sections: readonly Section[], bpm: number, bars: number) {
  const bytes = readFileSync(path);
  assert.equal(bytes.toString("ascii", 0, 4), "RIFF");
  let dataOffset = 0,
    dataBytes = 0;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const kind = bytes.toString("ascii", offset, offset + 4),
      length = bytes.readUInt32LE(offset + 4);
    if (kind === "fmt ") {
      assert.equal(bytes.readUInt16LE(offset + 10), 2);
      assert.equal(bytes.readUInt32LE(offset + 12), 48000);
      assert.equal(bytes.readUInt16LE(offset + 22), 24);
    }
    if (kind === "data") {
      dataOffset = offset + 8;
      dataBytes = length;
      break;
    }
    offset += 8 + length + (length % 2);
  }
  assert(dataOffset && dataOffset + dataBytes <= bytes.length);
  return sections.map(([name, start], index) => {
    const end = sections[index + 1]?.[1] ?? bars;
    const first = Math.round(((start * 4 * 60) / bpm) * 48000),
      last = Math.round(((end * 4 * 60) / bpm) * 48000);
    let energy = 0,
      peak = 0,
      leftEnergy = 0,
      rightEnergy = 0,
      cross = 0,
      sum = 0;
    let lowMid = 0,
      lowSide = 0;
    const filters = [
        [0, 0, 0],
        [0, 0, 0],
      ],
      bands = [0, 0, 0, 0];
    const coeffs = [150, 1000, 6000].map((hz) => 1 - Math.exp((-2 * Math.PI * hz) / 48000));
    for (let frame = first; frame < last; frame++) {
      const lr = [0, 1].map((ch) => bytes.readIntLE(dataOffset + frame * 6 + ch * 3, 3) / 8388608);
      for (const [ch, value] of lr.entries()) {
        energy += value * value;
        sum += value;
        peak = Math.max(peak, Math.abs(value));
        const state = filters[ch]!;
        for (let i = 0; i < 3; i++) state[i] = state[i]! + coeffs[i]! * (value - state[i]!);
        const split = [state[0]!, state[1]! - state[0]!, state[2]! - state[1]!, value - state[2]!];
        for (let i = 0; i < 4; i++) bands[i] = bands[i]! + split[i]! ** 2;
      }
      leftEnergy += lr[0]! ** 2;
      rightEnergy += lr[1]! ** 2;
      cross += lr[0]! * lr[1]!;
      lowMid += ((filters[0]![0]! + filters[1]![0]!) * 0.5) ** 2;
      lowSide += ((filters[0]![0]! - filters[1]![0]!) * 0.5) ** 2;
    }
    const rmsDbfs = 10 * Math.log10(Math.max(1e-20, energy / ((last - first) * 2)));
    const peakDbfs = 20 * Math.log10(Math.max(1e-10, peak));
    return {
      name,
      startBar: start + 1,
      bars: end - start,
      rmsDbfs,
      peakDbfs,
      crestDb: peakDbfs - rmsDbfs,
      dc: sum / ((last - first) * 2),
      stereoCorrelation: cross / Math.sqrt(Math.max(1e-30, leftEnergy * rightEnergy)),
      lowSideToMidDb: 10 * Math.log10(Math.max(1e-20, lowSide) / Math.max(1e-20, lowMid)),
      // Broad first-order analysis bands, overlapping slopes; not brick-wall FFT bins.
      bandRmsDbfs: Object.fromEntries(
        ["below150", "150to1000", "1000to6000", "above6000"].map((label, i) => [
          label,
          10 * Math.log10(Math.max(1e-20, bands[i]! / ((last - first) * 2))),
        ]),
      ),
    };
  });
}
