import { PREVIEW_MAX_FRAME_BYTES } from "@oxitone/protocol";

export function encodeFrame(value: unknown): Buffer {
  const body = Buffer.from(JSON.stringify(value));
  if (body.length === 0 || body.length > PREVIEW_MAX_FRAME_BYTES) throw new Error("Preview frame exceeds 64 MiB");
  const header = Buffer.alloc(4);
  header.writeUInt32BE(body.length);
  return Buffer.concat([header, body]);
}

/** Incremental framing with a single bounded allocation per payload. */
export class FrameDecoder {
  private header = Buffer.alloc(4);
  private headerUsed = 0;
  private body: Buffer | undefined;
  private used = 0;
  push(chunk: Buffer): unknown[] {
    const values: unknown[] = [];
    let cursor = 0;
    while (cursor < chunk.length) {
      if (!this.body) {
        const count = Math.min(4 - this.headerUsed, chunk.length - cursor);
        chunk.copy(this.header, this.headerUsed, cursor, cursor + count);
        cursor += count;
        this.headerUsed += count;
        if (this.headerUsed < 4) continue;
        const length = this.header.readUInt32BE();
        if (length === 0 || length > PREVIEW_MAX_FRAME_BYTES) throw new Error("Invalid preview frame length");
        this.body = Buffer.allocUnsafe(length);
      }
      const count = Math.min(this.body.length - this.used, chunk.length - cursor);
      chunk.copy(this.body, this.used, cursor, cursor + count);
      cursor += count;
      this.used += count;
      if (this.used === this.body.length) {
        values.push(JSON.parse(this.body.toString("utf8")));
        this.body = undefined;
        this.used = 0;
        this.headerUsed = 0;
      }
    }
    return values;
  }
}
