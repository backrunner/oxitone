import { describe, expect, it } from "vitest";
import { Project } from "../src/index.js";

describe("channel auxiliary outputs", () => {
  it("validates before mutation and restores routes without changing plugin identity", () => {
    const project = new Project();
    const first = project.addMixerChannel();
    const second = project.addMixerChannel();
    const routes = { "1": first.id };
    const channel = project.addChannel({ outputRoutes: routes });
    const instanceId = channel.instrumentInstance.id;
    routes["1"] = second.id;
    const returned = channel.outputRoutes;
    returned["1"] = second.id;
    expect(channel.outputRoutes).toEqual({ "1": first.id });
    const revision = project.revisionBigInt;
    for (const invalid of [{ "01": first.id }, { "16": first.id }, { "1": "mix_missing" }]) {
      expect(() => {
        channel.outputRoutes = invalid;
      }).toThrow();
      expect(project.revisionBigInt).toBe(revision);
      expect(channel.outputRoutes).toEqual({ "1": first.id });
      expect(channel.instrumentInstance.id).toBe(instanceId);
    }
    channel.outputRoutes = { "1": second.id, "2": first.id };
    expect(project.revisionBigInt).toBe(revision + 1n);
    expect(channel.instrumentInstance.id).toBe(instanceId);
    const restored = Project.fromSnapshot(project.snapshot());
    expect(restored.snapshot()).toEqual(project.snapshot());
    channel.outputRoutes = {};
    expect(channel.outputRoutes).toEqual({});
  });

  it("rejects dangling destinations at creation and unsupported built-in outputs at compilation", async () => {
    const project = new Project();
    const bus = project.addMixerChannel();
    expect(() => project.addChannel({ outputRoutes: { "1": "mix_missing" } })).toThrow();
    project.addChannel({ outputRoutes: { "1": bus.id } });
    await expect(project.compile()).rejects.toThrow();
  });
});
