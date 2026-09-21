import { expect, it, vi } from "vitest";
import { scanVst3Bundles } from "@oxitone/vst3";
import { Vst3Workbench } from "../src/source/plugins/vst3-workbench.js";

vi.mock("@oxitone/vst3", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@oxitone/vst3")>()),
  scanVst3Bundles: vi.fn(),
}));

it("publishes scans atomically without loading plugins, and keeps prior results after cancel or stale revision", async () => {
  const inspect = vi.fn(),
    render = vi.fn();
  const workbench = new Vst3Workbench("/project", { inspect, render });
  const scan = vi.mocked(scanVst3Bundles);
  const signal = new AbortController().signal;
  scan.mockResolvedValue(["/Library/Audio/Plug-Ins/VST3/Fixture.vst3"]);
  expect(workbench.scannedBundles).toBeUndefined();
  await workbench.scan(signal, () => {});
  const accepted = workbench.scannedBundles!;
  accepted.push("/mutated");
  expect(workbench.scannedBundles).toHaveLength(1);
  scan.mockResolvedValue([]);
  await expect(
    workbench.scan(signal, () => {
      throw new Error("stale");
    }),
  ).rejects.toThrow("stale");
  expect(workbench.scannedBundles).toHaveLength(1);
  scan.mockImplementation(async (options) => {
    workbench.cancel();
    options!.signal!.throwIfAborted();
    return [];
  });
  await expect(workbench.scan(signal, () => {})).rejects.toMatchObject({ name: "AbortError" });
  expect(workbench.scannedBundles).toHaveLength(1);
  expect(inspect).not.toHaveBeenCalled();
  expect(render).not.toHaveBeenCalled();
});
