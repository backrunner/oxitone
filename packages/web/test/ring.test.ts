import { describe, expect, it } from "vitest";
import { PcmRing, RUNNING, READ, WRITE } from "../src/ring.js";
import { PresentationCursor } from "../src/presentation.js";
describe("shared PCM output", () => {
  it("tracks consumed frames without skipping the initial span before a loop or counting flushed PCM", () => {
    const ring = PcmRing.create(512),
      cursor = new PresentationCursor();
    cursor.loop = { startFrame: 128, endFrame: 256 };
    cursor.reset(ring, 0n);
    const signal = new Float32Array(128),
      left = new Float32Array(128),
      right = new Float32Array(128);
    ring.write(signal, signal);
    Atomics.store(ring.header, RUNNING, 1);
    expect(cursor.current(ring)).toBe(0n);
    ring.read(left, right);
    expect(cursor.current(ring)).toBe(128n);
    ring.write(signal, signal);
    ring.read(left, right);
    expect(cursor.current(ring)).toBe(128n);
    ring.write(signal, signal);
    ring.flush();
    cursor.reset(ring, 500n);
    expect(cursor.current(ring)).toBe(500n);
    ring.read(left, right);
    expect(cursor.current(ring)).toBe(500n);
  });
  it("flushes through the consumer before accepting new data, and zeros underruns", () => {
    const ring = PcmRing.create(512),
      l = new Float32Array(128),
      r = new Float32Array(128);
    const signal = new Float32Array(128).fill(0.5);
    expect(ring.write(signal, signal)).toBe(true);
    Atomics.store(ring.header, RUNNING, 1);
    ring.read(l, r);
    expect([...l]).toEqual([...signal]);
    ring.flush();
    expect(ring.write(signal, signal)).toBe(false);
    ring.read(l, r);
    expect(l.every((x) => x === 0)).toBe(true);
    expect(ring.write(signal, signal)).toBe(true);
    Atomics.store(ring.header, RUNNING, 1);
    ring.read(l, r);
    ring.read(l, r);
    expect(l.every((x) => x === 0)).toBe(true);
    expect(Atomics.load(ring.header, 3)).toBe(1);
  });
  it("preserves channel order across u32 cursor wrap and rejects overwrite", () => {
    const ring = PcmRing.create(512);
    Atomics.store(ring.header, READ, -128);
    Atomics.store(ring.header, WRITE, -128);
    const l = Float32Array.from({ length: 512 }, (_, i) => i),
      r = l.map((v) => -v);
    expect(ring.write(l, r)).toBe(true);
    expect(ring.write(l, r)).toBe(false);
    Atomics.store(ring.header, RUNNING, 1);
    const outL = new Float32Array(512),
      outR = new Float32Array(512);
    ring.read(outL, outR);
    expect(outL).toEqual(l);
    expect(outR).toEqual(r);
    expect(ring.buffered()).toBe(0);
  });
});
