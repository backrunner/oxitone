import { describe, expect, it, vi } from "vitest";
import { vst3RenderOptionsSchema, vst3InfoSchema, type Vst3Info } from "@oxitone/protocol";
import { Vst3Workbench } from "../src/source/plugins/vst3-workbench.js";

const source = { bundlePath: "/tmp/Example.vst3", classId: "1".repeat(32) };
const info = (): Vst3Info =>
  vst3InfoSchema.parse({
    protocolVersion: 1,
    classId: source.classId,
    name: "Gain",
    vendor: "Fixture",
    version: "1",
    category: "Fx",
    sha256: "a".repeat(64),
    inputChannels: 2,
    outputChannels: 2,
    audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [{ channels: 2, active: true }] },
    noteInput: false,
    noteOutput: false,
    parameters: [
      { id: 9, name: "Gain", unit: "dB", value: 0.5, default: 0.5, stepCount: 0, canAutomate: true, readOnly: false },
      { id: 10, name: "Meter", unit: "", value: 0, default: 0, stepCount: 0, canAutomate: false, readOnly: true },
    ],
    configuration: {
      formatVersion: 1,
      classId: source.classId,
      sha256: "a".repeat(64),
      stateBase64: "",
      parameters: { "9": 0.5 },
    },
  });
const options = () => vst3RenderOptionsSchema.parse({ path: "result.wav", inputPath: "input.wav", frames: 128 });
const signal = () => new AbortController().signal;
const check = () => {};
function fixture() {
  const inspect = vi.fn(async () => info());
  const render = vi.fn(async () => ({
    protocolVersion: 1 as const,
    path: "/project/result.wav",
    frames: 128,
    sampleRate: 48000,
    sha256: "b".repeat(64),
    pluginSha256: "a".repeat(64),
    peak: 0.1,
    latencyFrames: 0,
    tailFrames: 0,
  }));
  const workbench = new Vst3Workbench("/project", { inspect, render });
  const plugin = workbench.add(source);
  return { workbench, plugin, inspect, render };
}
describe("VST3 session control", () => {
  it("keeps Cancel unchanged and retains applied editor state through refresh and inspection", async () => {
    const inspect = vi.fn(async () => info());
    const edit = vi.fn(async (): Promise<Vst3Info | null> => null);
    const configure = vi.fn(async (_source, options) => ({ ...info(), configuration: options.configuration }));
    const { render } = fixture();
    const workbench = new Vst3Workbench("/project", { inspect, render, edit, configure });
    const plugin = workbench.add(source);
    await workbench.inspect(plugin, "any", signal(), check);
    const before = structuredClone(plugin);
    await workbench.edit(plugin, { "9": 0.2 }, "any", signal(), check);
    expect(plugin).toEqual(before);
    const changed = info();
    changed.configuration!.parameters["9"] = 0.2;
    changed.configuration!.stateBase64 = "AQID";
    edit.mockResolvedValue(changed);
    await workbench.edit(plugin, { "9": 0.2 }, "any", signal(), check);
    expect(plugin.vst3Info!.configuration!.stateBase64).toBe("AQID");
    const refreshed = workbench.refresh()[0]!;
    await workbench.inspect(refreshed, "any", signal(), check);
    expect(refreshed.vst3Info!.configuration).toEqual(changed.configuration);
    expect(refreshed.entry.vst3!.parameters[0]!.value).toBe(0.2);
    expect(configure).toHaveBeenCalledWith(
      expect.objectContaining({ allowPlugins: "any" }),
      { configuration: changed.configuration },
      expect.anything(),
    );
    const accepted = structuredClone(refreshed);
    edit.mockResolvedValue({ ...changed, vendor: "x".repeat(8 * 1024 * 1024) });
    await expect(workbench.edit(refreshed, {}, "any", signal(), check)).rejects.toMatchObject({
      code: "BudgetExceeded",
    });
    expect(refreshed).toEqual(accepted);
  });
  it("adds without executing native code, pins inspected content/state and inherits the explicit project policy", async () => {
    const { workbench, plugin, inspect, render } = fixture();
    expect(inspect).not.toHaveBeenCalled();
    expect(workbench.add(source)).toBe(plugin);
    await expect(workbench.render(plugin, options(), "signed-only", signal(), check)).rejects.toMatchObject({
      code: "PluginConfigInvalid",
    });
    await workbench.inspect(plugin, "any", signal(), check);
    expect(inspect).toHaveBeenCalledWith(expect.objectContaining({ allowPlugins: "any" }), expect.anything());
    await workbench.render(plugin, { ...options(), parameters: { "9": 0.2 } }, "signed-only", signal(), check);
    expect(render).toHaveBeenCalledWith(
      expect.objectContaining({ expectedHash: "a".repeat(64), allowPlugins: "signed-only" }),
      expect.objectContaining({
        path: "/project/result.wav",
        inputPath: "/project/input.wav",
        parameters: { "9": 0.2 },
        configuration: info().configuration,
      }),
      expect.anything(),
    );
    expect(plugin.entry.vst3?.render?.peak).toBe(0.1);
    expect(plugin.entry.vst3).not.toHaveProperty("configuration");
  });
  it("rejects read-only/unknown parameters and unsupported note input before starting native work", async () => {
    const { workbench, plugin, render } = fixture();
    await workbench.inspect(plugin, "any", signal(), check);
    for (const id of ["10", "999"]) {
      await expect(
        workbench.render(plugin, { ...options(), parameters: { [id]: 0.5 } }, "any", signal(), check),
      ).rejects.toMatchObject({ code: "PluginConfigInvalid" });
    }
    await expect(
      workbench.render(
        plugin,
        {
          ...options(),
          events: [{ type: "noteOn", channel: 0, pitch: 60, frame: 0, velocity: 1 }],
        },
        "any",
        signal(),
        check,
      ),
    ).rejects.toMatchObject({ code: "PluginCapabilityUnsupported" });
    expect(render).not.toHaveBeenCalled();
  });
  it("refresh expires configuration, preserves local selections and rejects stale inspection completion", async () => {
    const { workbench, plugin } = fixture();
    await expect(
      workbench.inspect(plugin, "any", signal(), () => {
        throw new Error("stale");
      }),
    ).rejects.toThrow("stale");
    expect(plugin.entry.validation).not.toBe("verified");
    await workbench.inspect(plugin, "any", signal(), check);
    const refreshed = workbench.refresh()[0]!;
    expect(refreshed.entry.validation).toBe("unverified");
    expect(refreshed.entry.vst3?.parameters).toEqual([]);
    await expect(workbench.render(refreshed, options(), "any", signal(), check)).rejects.toMatchObject({
      code: "PluginConfigInvalid",
    });
    workbench.remove(refreshed.entry.handle);
    expect(workbench.refresh()).toEqual([]);
  });
  it("cancels active inspection and permits a later retry", async () => {
    let started!: () => void;
    const ready = new Promise<void>((resolve) => {
      started = resolve;
    });
    const inspect = vi.fn(
      (_source, host) =>
        new Promise<Vst3Info>((_resolve, reject) => {
          host?.signal?.addEventListener("abort", () => reject(new DOMException("Aborted", "AbortError")), {
            once: true,
          });
          started();
        }),
    );
    const { render } = fixture();
    const workbench = new Vst3Workbench("/project", { inspect, render });
    const plugin = workbench.add(source);
    const pending = workbench.inspect(plugin, "any", signal(), check);
    const rejected = expect(pending).rejects.toMatchObject({ name: "AbortError" });
    await ready;
    workbench.cancel();
    await rejected;
    expect(plugin.entry.validation).toBe("failed");
    inspect.mockResolvedValue(info());
    await workbench.inspect(plugin, "any", signal(), check);
    expect(plugin.entry.validation).toBe("verified");
  });
  it("bounds local entries and opaque states, and releases their budgets on refresh", async () => {
    const { workbench, inspect } = fixture();
    const captured = info();
    captured.configuration!.stateBase64 = Buffer.alloc(4 * 1024 * 1024).toString("base64");
    inspect.mockResolvedValue(captured);
    for (let i = 1; i < 32; i++) workbench.add({ ...source, bundlePath: `/tmp/Fixture-${i}.vst3` });
    expect(() => workbench.add({ ...source, bundlePath: "/tmp/Overflow.vst3" })).toThrow(/32/);
    const plugins = workbench.refresh();
    for (const plugin of plugins.slice(0, 4)) await workbench.inspect(plugin, "any", signal(), check);
    await expect(workbench.inspect(plugins[4]!, "any", signal(), check)).rejects.toMatchObject({
      code: "BudgetExceeded",
    });
    expect(plugins[4]!.entry.validation).toBe("failed");
    const refreshed = workbench.refresh();
    await expect(workbench.inspect(refreshed[4]!, "any", signal(), check)).resolves.toBeUndefined();
  });
  it("bounds parameter projections and rejects changed class identity", async () => {
    const { workbench, plugin, inspect } = fixture();
    const captured = info();
    captured.parameters[0]!.name = "x".repeat(5 * 1024 * 1024);
    inspect.mockResolvedValue(captured);
    await workbench.inspect(plugin, "any", signal(), check);
    const second = workbench.add({ ...source, bundlePath: "/tmp/Second.vst3" });
    await expect(workbench.inspect(second, "any", signal(), check)).rejects.toMatchObject({ code: "BudgetExceeded" });
    workbench.remove(plugin.entry.handle);
    await expect(workbench.inspect(second, "any", signal(), check)).resolves.toBeUndefined();
    inspect.mockResolvedValue({ ...info(), classId: "2".repeat(32) });
    await expect(workbench.inspect(second, "any", signal(), check)).rejects.toMatchObject({
      code: "PluginManifestMismatch",
    });
    expect(second.entry.vst3?.parameters).toEqual([]);
  });
  it("keeps MIDI-aware effects in the effect category and budgets all inspected metadata", async () => {
    const { workbench, plugin, inspect } = fixture();
    inspect.mockResolvedValue({ ...info(), noteInput: true, noteOutput: false, category: "Fx|Delay" });
    await workbench.inspect(plugin, "any", signal(), check);
    expect(plugin.entry.kind).toBe("effect");
    inspect.mockResolvedValue({ ...info(), vendor: "x".repeat(8 * 1024 * 1024) });
    await expect(workbench.inspect(plugin, "any", signal(), check)).rejects.toMatchObject({ code: "BudgetExceeded" });
  });
});
