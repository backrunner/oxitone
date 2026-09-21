import { expect, it } from "vitest";
import { AutomationSource } from "../src/automation/source.js";
import { recordedSource } from "../src/automation/recording-source.js";
import { RecordingTakeBuilder } from "../src/automation/recording-take.js";
import type { Vst3EditPage } from "@oxitone/protocol";

const target = { graphGeneration: "1", instanceId: "ins_gain" };
const recording = { mode: "write" as const, parameterIds: [0], sampleRate: 48000 };
function page(first: number, count: number, next = first + count): Vst3EditPage {
  return {
    captureId: "1",
    status: "recording",
    recording,
    firstSequence: first,
    nextSequence: next,
    pendingEvents: 0,
    events: Array.from({ length: count }, (_, index) => {
      const sequence = first + index;
      return {
        sequence,
        kind: "sample" as const,
        parameterId: 0,
        value: 0.75,
        frames: 128,
        position: {
          audioSequence: sequence,
          reset: false,
          transport: {
            projectFrame: sequence * 128,
            continuousFrame: sequence * 128,
            projectBeat: (sequence * 128) / 24000,
            barBeat: 0,
            tempo: 120,
            timeSignature: [4, 4],
            playing: true,
          },
        },
      };
    }),
  };
}
it("drains by the returned page length and merges held values without losing audio spans", () => {
  const builder = new RecordingTakeBuilder(target, "1", recording);
  builder.accept(page(0, 256, 512));
  expect(builder.cursor).toBe(256);
  expect(() => builder.finish()).toThrow(/not fully stopped/);
  builder.accept(page(256, 256, 512));
  const end = page(512, 1).events[0]!.position;
  builder.accept({ ...page(512, 0, 512), status: "stopped", endPosition: end });
  const take = builder.finish();
  expect(take.spans).toHaveLength(1);
  expect(take.spans[0]).toMatchObject({ start: 0, end: (512 * 128) / 24000, value: 0.75 });
  expect(Object.isFrozen(take.spans[0])).toBe(true);
  expect(() => take.source(99, new AutomationSource({ kind: "constant", value: 0.2 }))).toThrow();
});
it("rejects a lost page, changed capture, failed capture and stale clock", () => {
  for (const bad of [{ ...page(0, 1), captureId: "2" }, { ...page(0, 1), recording: undefined }, page(1, 1)]) {
    expect(() => new RecordingTakeBuilder(target, "1", recording).accept(bad)).toThrow();
  }
  const builder = new RecordingTakeBuilder(target, "1", recording);
  builder.accept(page(0, 2));
  expect(() =>
    builder.accept({
      ...page(2, 0),
      status: "failed",
      error: { code: "BudgetExceeded", message: "vendor events lost" },
    }),
  ).toThrow(/lost/);
  expect(() => builder.finish()).toThrow();
  const backwards = page(2, 1);
  backwards.events[0]!.position.audioSequence = 0;
  expect(() => builder.accept(backwards)).toThrow(/backwards/);
});
it("later loop passes overlay only touched intervals and preserve the base generator", () => {
  const base = new AutomationSource({ kind: "wave", wave: "sine", periodBeats: { numerator: 4, denominator: 1 } });
  const span = (start: number, end: number, value: number) => ({ parameterId: 0, start, end, value });
  const source = recordedSource([span(1, 5, 0.2), span(2, 3, 0.8), span(3, 4, 0.8), span(7, 8, 0.6)], 0, base).toSpec();
  expect(source).toMatchObject({
    kind: "replaceRange",
    startBeat: { numerator: 7, denominator: 1 },
    endBeat: { numerator: 8, denominator: 1 },
  });
  if (source.kind !== "replaceRange" || source.base.kind !== "replaceRange") throw Error("missing overlays");
  expect(source.base.base).toEqual(base.toSpec());
  expect(source.base.replacement).toEqual({
    kind: "curve",
    interpolation: "linear",
    points: [
      { beat: { numerator: 0, denominator: 1 }, value: 0.2, curve: { kind: "step" } },
      { beat: { numerator: 1, denominator: 1 }, value: 0.8, curve: { kind: "step" } },
      { beat: { numerator: 3, denominator: 1 }, value: 0.2, curve: { kind: "step" } },
    ],
  });
});
