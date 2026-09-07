import { ACK, EPOCH, READ, WRITE, PcmRing } from "./ring.js";
import type { WasmTransport } from "./types.js";

/** Presentation cursor follows consumed PCM, including underruns and the initial pre-loop span. */
export class PresentationCursor {
  private frame = 0n;
  private lastRead = 0;
  loop: WasmTransport["loop"];
  reset(ring: PcmRing, frame: bigint): void {
    this.frame = frame;
    this.lastRead = Atomics.load(ring.header, WRITE) >>> 0;
  }
  current(ring: PcmRing): bigint {
    if (Atomics.load(ring.header, ACK) !== Atomics.load(ring.header, EPOCH)) return this.frame;
    const read = Atomics.load(ring.header, READ) >>> 0;
    const consumed = (read - this.lastRead) >>> 0;
    if (consumed === 0) return this.frame;
    this.frame += BigInt(consumed);
    this.lastRead = read;
    if (this.loop && this.frame >= BigInt(this.loop.endFrame)) {
      const start = BigInt(this.loop.startFrame), end = BigInt(this.loop.endFrame);
      this.frame = start + (this.frame - end) % (end - start);
    }
    return this.frame;
  }
}
