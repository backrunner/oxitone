import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

export async function pcm(path) {
  const bytes = await readFile(path);
  assert.equal(bytes.toString("ascii", 0, 4), "RIFF");
  let data;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const size = bytes.readUInt32LE(offset + 4);
    if (bytes.toString("ascii", offset, offset + 4) === "fmt ") {
      assert.equal(bytes.readUInt16LE(offset + 10), 2);
      assert.equal(bytes.readUInt32LE(offset + 12), 48000);
      assert.equal(bytes.readUInt16LE(offset + 22), 32);
    }
    if (bytes.toString("ascii", offset, offset + 4) === "data") data = bytes.subarray(offset + 8, offset + 8 + size);
    offset += 8 + size + (size % 2);
  }
  assert.ok(data);
  return Array.from({ length: data.length / 4 }, (_, i) => data.readFloatLE(i * 4));
}

export function compare(actual, baseline, gain, label) {
  assert.equal(actual.length, baseline.length);
  let peakError = 0;
  let peak = 0;
  for (let i = 0; i < actual.length; i++) {
    peakError = Math.max(peakError, Math.abs(actual[i] - baseline[i] * gain));
    peak = Math.max(peak, Math.abs(actual[i]));
  }
  assert.ok(peak > 0.001, `${label}: output must contain audio`);
  assert.ok(peakError < 0.000002, `${label}: PCM error ${peakError}`);
  return { peakError, samples: actual.length };
}
