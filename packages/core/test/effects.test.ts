import { describe, expect, it } from "vitest";
import { createEngine, dispose, getPluginInfo } from "@oxitone/native";
import { effectParameterSchemas, effectPluginIds, ErrorCode, type EffectKind } from "@oxitone/protocol";
import { convolver, createAutomationNamespace, effect, Pattern, Project } from "../src/index.js";

describe("electronic effect authoring and native contracts", () => {
  it("validates physical parameters and detaches caller options", () => {
    const parameters = { cutoffHz: 1400, driveDb: 12 };
    const ref = effect("nonlinearFilter", parameters, { mix: 0.3 });
    parameters.cutoffHz = 20;
    expect(ref).toMatchObject({ pluginId: "oxitone.nonlinear-filter", parameters: { cutoffHz: 1400 }, mix: 0.3 });
    expect(() => effect("nonlinearFilter", { mode: 1.5 })).toThrow();
    expect(() => effect("pitchShifter", { semitones: 25 })).toThrow();
    expect(() => effect("limiter", { ceilingDb: NaN })).toThrow();
    expect(() => effect("tape", {}, { mix: 2 })).toThrow();
    expect(convolver("smp_room").resources).toEqual({ impulse: "smp_room" });
    expect(effect("limiter", { ceilingDb: undefined })).toEqual({ pluginId: "oxitone.limiter", pluginVersion: "1.0.0", parameters: {} });
  });

  it("keeps every typed parameter and range in agreement with the native descriptor", () => {
    const engine = createEngine({ audioBackend: "simulated" });
    try {
      for (const kind of Object.keys(effectPluginIds) as EffectKind[]) {
        const descriptor = getPluginInfo(engine, effectPluginIds[kind], "1.0.0");
        const schema = effectParameterSchemas[kind];
        expect(Object.keys(schema.shape).sort()).toEqual(descriptor.parameters.map(p => p.id).sort());
        for (const spec of descriptor.parameters) {
          for (const value of [spec.min, spec.default, spec.max]) expect(schema.safeParse({ [spec.id]: value }).success, `${kind}.${spec.id}=${value}`).toBe(true);
          for (const value of [spec.min - 1, spec.max + 1, NaN, Infinity]) expect(schema.safeParse({ [spec.id]: value }).success).toBe(false);
        }
        expect(schema.safeParse({ unknown: 1 }).success).toBe(false);
      }
    } finally { dispose(engine); }
  });

  it("compiles every processor with insert mix and parameter automation through the native facade", async () => {
    const project = new Project({ seed: 19 });
    const channel = project.addChannel();
    project.addTrack().use(channel).add(new Pattern({ lengthBeats: 1, notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.5 }] })).at({ bar: 1 });
    try {
      for (const kind of Object.keys(effectPluginIds) as EffectKind[]) {
        channel.effectChain = [effect(kind, {}, { mix: 0.5 })];
        await project.compile({ audioBackend: "simulated" });
      }
      channel.effectChain = [effect("nonlinearFilter", { cutoffHz: 400 })];
      channel.automate("insert.0.parameter.cutoffHz", createAutomationNamespace().sine({ periodBeats: 1 }));
      await project.compile({ audioBackend: "simulated" });
      channel.effectChain = [convolver("smp_missing")];
      await expect(project.compile({ audioBackend: "simulated" })).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
    } finally { await project.session?.dispose(); }
  }, 120000);
});
