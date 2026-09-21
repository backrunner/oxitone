import { afterEach, expect, it, vi } from "vitest";
import type { Vst3EditPage, Vst3InstanceResult, Vst3ControlCommand } from "@oxitone/protocol";
import { Vst3AutomationRecorder } from "../src/engine/vst3-recording.js";
import type { Session } from "../src/engine/session.js";

afterEach(() => vi.useRealTimers());
const target = { graphGeneration: "7", instanceId: "ins_gain" };
const page: Vst3EditPage = {
  captureId: "2",
  status: "recording",
  firstSequence: 0,
  nextSequence: 0,
  pendingEvents: 0,
  events: [],
  recording: { mode: "write", parameterIds: [0], sampleRate: 48000 },
};
const stopped: Vst3EditPage = {
  ...page,
  status: "stopped",
  endPosition: {
    audioSequence: 1,
    reset: false,
    transport: {
      projectFrame: 128,
      continuousFrame: 128,
      projectBeat: 128 / 24000,
      barBeat: 0,
      tempo: 120,
      timeSignature: [4, 4],
      playing: true,
    },
  },
};
function harness() {
  const commands: Vst3ControlCommand[] = [];
  const targets: (typeof target)[] = [];
  let resolveRead!: (value: Vst3InstanceResult) => void;
  const pending = new Promise<Vst3InstanceResult>((resolve) => {
    resolveRead = resolve;
  });
  const result = (edits: Vst3EditPage) => ({ state: { edits } }) as Vst3InstanceResult;
  const session = {
    controlVst3Instance: vi.fn(async (current: typeof target, command: Vst3ControlCommand) => {
      commands.push(command);
      targets.push(current);
      return command.kind === "readEdits" ? pending : result(page);
    }),
  } as unknown as Session;
  return { session, commands, targets, resolve: () => resolveRead(result(stopped)) };
}
it("cancel during an in-flight final page never publishes a take and cleans only the original graph", async () => {
  const test = harness();
  const mutableTarget = { ...target };
  const recorder = await Vst3AutomationRecorder.start(test.session, mutableTarget, {
    mode: "write",
    parameterIds: [0],
  });
  mutableTarget.graphGeneration = "8";
  const cancel = recorder.cancel();
  test.resolve();
  await cancel;
  await expect(recorder.stop()).rejects.toMatchObject({ code: "PluginTaskConflict" });
  expect(recorder.active).toBe(false);
  expect(test.commands.at(-1)).toEqual({ kind: "discardEdits", captureId: "2" });
  expect(test.targets.at(-1)).toEqual(target);
});
it("a final page arriving after the stop deadline cannot be accepted", async () => {
  vi.useFakeTimers({ toFake: ["performance"] });
  const test = harness();
  const recorder = await Vst3AutomationRecorder.start(test.session, target, { mode: "write", parameterIds: [0] });
  const result = expect(recorder.stop({ timeoutMs: 10 })).rejects.toMatchObject({ code: "PluginTaskConflict" });
  vi.advanceTimersByTime(11);
  test.resolve();
  await result;
  expect(recorder.error?.message).toMatch(/deadline/);
  expect(recorder.active).toBe(false);
  expect(test.commands.at(-1)?.kind).toBe("discardEdits");
});
