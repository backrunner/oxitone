import { describe, expect, it } from "vitest";
import { ErrorCode, vst3RenderOptionsSchema, vst3SourceSchema } from "@oxitone/protocol";
import { inspectVst3Plugin, renderVst3Wav } from "../src/index.js";

const source = {
  bundlePath: "/tmp/Fixture.vst3",
  classId: "56455354494741494e30303030303031",
  allowPlugins: "any" as const,
};

describe("optional VST3 host boundary", () => {
  it("requires an exact local source and canonical normalized values", () => {
    expect(vst3SourceSchema.parse(source)).toMatchObject(source);
    expect(() => vst3SourceSchema.parse({ ...source, classId: "bad" })).toThrow();
    expect(() => vst3SourceSchema.parse({ ...source, bundlePath: "relative.vst3" })).not.toThrow();
  });

  it("separates schema shape from content-frame validation", () => {
    const options = vst3RenderOptionsSchema.parse({ path: "/tmp/out.wav", frames: 128 });
    expect(options.sampleRate).toBe(48_000);
    expect(() =>
      vst3RenderOptionsSchema.parse({
        path: "/tmp/out.wav",
        frames: 128,
        events: [{ type: "noteOn", frame: 128, channel: 0, pitch: 60, velocity: 1 }],
      }),
    ).not.toThrow();
    expect(() => vst3RenderOptionsSchema.parse({ path: "/tmp/out.wav", frames: 0 })).toThrow();
  });

  it("rejects events at content end before starting the helper", async () => {
    await expect(
      renderVst3Wav(source, {
        path: "/tmp/out.wav",
        frames: 128,
        events: [{ type: "noteOn", frame: 128, channel: 0, pitch: 60, velocity: 1 }],
      }),
    ).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
  });

  it("fails closed when the optional native helper is not installed", async () => {
    await expect(
      inspectVst3Plugin(source, { hostPath: "/definitely/missing/oxitone-vst3-host" }),
    ).rejects.toMatchObject({
      code: ErrorCode.PluginHostUnavailable,
    });
  });
});
