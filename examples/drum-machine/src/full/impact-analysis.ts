import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

/** Read only offline PCM24; event windows complement whole-section loudness measurements. */
export function inspectImpact(path: string, bpm = 140, startBars: readonly number[] = [24, 72], bars = 16) {
  const wav = readFileSync(path); let offset = 12, data = 0;
  assert.equal(wav.toString("ascii", 0, 4), "RIFF");
  while (offset + 8 <= wav.length) {
    const kind = wav.toString("ascii", offset, offset + 4), length = wav.readUInt32LE(offset + 4);
    if (kind === "fmt ") {
      assert.equal(wav.readUInt32LE(offset + 12), 48000); assert.equal(wav.readUInt16LE(offset + 10), 2);
      assert.equal(wav.readUInt16LE(offset + 22), 24);
    }
    if (kind === "data") { data = offset + 8; break; }
    offset += 8 + length + length % 2;
  }
  assert(data);
  const window = (beat: number, from: number, to: number) => {
    const start = Math.max(0, Math.round((beat * 60 / bpm + from) * 48000));
    const end = Math.round((beat * 60 / bpm + to) * 48000);
    assert(data + end * 6 <= wav.length);
    let energy = 0, middle = 0, peak = 0;
    const low = [0, 0], high = [0, 0], a = 1 - Math.exp(-2 * Math.PI * 150 / 48000), b = 1 - Math.exp(-2 * Math.PI * 6000 / 48000);
    for (let i = Math.max(0, start - 2400); i < end; i++) for (let ch = 0; ch < 2; ch++) {
      const v = wav.readIntLE(data + i * 6 + ch * 3, 3) / 8388608;
      low[ch] = low[ch]! + a * (v - low[ch]!); high[ch] = high[ch]! + b * (v - high[ch]!);
      if (i >= start) { energy += v * v; middle += (high[ch]! - low[ch]!) ** 2; peak = Math.max(peak, Math.abs(v)); }
    }
    return { rmsDbfs: 10 * Math.log10(Math.max(1e-20, energy / ((end - start) * 2))),
      midRmsDbfs: 10 * Math.log10(Math.max(1e-20, middle / ((end - start) * 2))),
      peakDbfs: 20 * Math.log10(Math.max(1e-10, peak)) };
  };
  return startBars.map(startBar => {
    const hits = Array.from({ length: bars }, (_, i) => {
      const beat = (startBar + i) * 4 + 2, before = window(beat, -0.22, -0.035), after = window(beat, 0.015, 0.2);
      return { bar: startBar + i + 1, before, after, midLiftDb: after.midRmsDbfs - before.midRmsDbfs };
    });
    const sorted = hits.map(h => h.midLiftDb).sort((a, b) => a - b);
    const gap = startBar > 0 ? window(startBar * 4, -0.28, -0.04) : undefined;
    const arrival = window(startBar * 4, 0, 0.24);
    return { startBar: startBar + 1, preDropGap: gap, arrival,
      arrivalLiftDb: gap ? arrival.rmsDbfs - gap.rmsDbfs : undefined,
      snareMedianMidLiftDb: (sorted[Math.floor((bars - 1) / 2)]! + sorted[Math.floor(bars / 2)]!) / 2, snares: hits };
  });
}
