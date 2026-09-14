/** Single producer/consumer PCM ring. Unsigned cursors wrap at 2^32. */
export const WRITE = 0,
  READ = 1,
  RUNNING = 2,
  UNDERRUNS = 3,
  EPOCH = 4,
  ACK = 5,
  FLUSH_AT = 6;
export const HEADER_WORDS = 8;
export class PcmRing {
  readonly header: Int32Array;
  readonly left: Float32Array;
  readonly right: Float32Array;
  constructor(
    readonly buffer: SharedArrayBuffer,
    readonly frames: number,
  ) {
    this.header = new Int32Array(buffer, 0, HEADER_WORDS);
    this.left = new Float32Array(buffer, HEADER_WORDS * 4, frames);
    this.right = new Float32Array(buffer, (HEADER_WORDS + frames) * 4, frames);
  }
  static create(frames: number): PcmRing {
    if (!Number.isInteger(frames) || frames < 512 || frames > 65536 || frames & (frames - 1))
      throw new RangeError("ringFrames must be a power of two between 512 and 65536");
    return new PcmRing(new SharedArrayBuffer((HEADER_WORDS + frames * 2) * 4), frames);
  }
  buffered(): number {
    if (Atomics.load(this.header, ACK) !== Atomics.load(this.header, EPOCH)) return 0;
    return ((Atomics.load(this.header, WRITE) >>> 0) - (Atomics.load(this.header, READ) >>> 0)) >>> 0;
  }
  /** Producer does not touch READ; consumer acknowledges before new PCM is published. */
  flush(): void {
    Atomics.store(this.header, RUNNING, 0);
    Atomics.store(this.header, FLUSH_AT, Atomics.load(this.header, WRITE));
    Atomics.add(this.header, EPOCH, 1);
  }
  write(left: Float32Array, right: Float32Array): boolean {
    if (
      Atomics.load(this.header, ACK) !== Atomics.load(this.header, EPOCH) ||
      left.length > this.frames - this.buffered()
    )
      return false;
    const write = Atomics.load(this.header, WRITE) >>> 0;
    for (let i = 0; i < left.length; i++) {
      const index = (write + i) & (this.frames - 1);
      this.left[index] = left[i]!;
      this.right[index] = right[i]!;
    }
    Atomics.store(this.header, WRITE, (write + left.length) | 0);
    return true;
  }
  /** Called only by the worklet. No allocation, blocking, DSP or authoring code. */
  read(left: Float32Array, right: Float32Array): void {
    const epoch = Atomics.load(this.header, EPOCH);
    if (Atomics.load(this.header, ACK) !== epoch) {
      Atomics.store(this.header, READ, Atomics.load(this.header, FLUSH_AT));
      Atomics.store(this.header, ACK, epoch);
    }
    left.fill(0);
    right.fill(0);
    if (!Atomics.load(this.header, RUNNING)) return;
    const read = Atomics.load(this.header, READ) >>> 0;
    const count = Math.min(left.length, this.buffered());
    for (let i = 0; i < count; i++) {
      const index = (read + i) & (this.frames - 1);
      left[i] = this.left[index]!;
      right[i] = this.right[index]!;
    }
    Atomics.store(this.header, READ, (read + count) | 0);
    if (count < left.length) Atomics.add(this.header, UNDERRUNS, 1);
    // A concurrent flush invalidates this entire quantum; producer waits for ACK.
    if (Atomics.load(this.header, EPOCH) !== epoch) {
      left.fill(0);
      right.fill(0);
    }
  }
}
