import { expect, it } from "vitest";
import { PREVIEW_MAX_FRAME_BYTES } from "@oxitone/protocol";
import { FrameDecoder, encodeFrame } from "../src/preview/framing.js";
import { parsePreviewArgs } from "../src/preview/launch.js";

it("decodes fragmented headers/bodies, UTF-8 and consecutive messages with bounded framing", () => {
  const values = [{ type: "diagnostic", message: "编译失败 🎹" }, { type: "query" }];
  const bytes = Buffer.concat(values.map(encodeFrame));
  for (const size of [1, 2, 3, 7, bytes.length]) {
    const decoder = new FrameDecoder();
    const result: unknown[] = [];
    for (let i = 0; i < bytes.length; i += size) result.push(...decoder.push(bytes.subarray(i, i + size)));
    expect(result).toEqual(values);
  }
  for (const length of [0, PREVIEW_MAX_FRAME_BYTES + 1]) {
    const header = Buffer.alloc(4);
    header.writeUInt32BE(length);
    expect(() => new FrameDecoder().push(header)).toThrow("length");
  }
  expect(() => new FrameDecoder().push(Buffer.from([0, 0, 0, 1, 120]))).toThrow();
});

it("validates preview options and defaults to dependency watch", () => {
  expect(parsePreviewArgs(["song.ts"]).options.watch).toBe(true);
  expect(parsePreviewArgs(["song.ts", "--no-watch", "--headless"]).options).toMatchObject({
    watch: false,
    headless: true,
  });
  expect(parsePreviewArgs(["song.ts", "--document-socket", "/tmp/oxitone-test/document"]).options.documentSocket).toBe(
    "/tmp/oxitone-test/document",
  );
  expect(() => parsePreviewArgs(["song.ts", "--document-socket"])).toThrow("requires a path");
  expect(() => parsePreviewArgs(["song.ts", "--viewer"])).toThrow("requires a path");
  expect(() => parsePreviewArgs(["song.ts", "--edit"])).toThrow("Unknown");
});
