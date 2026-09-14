import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { ErrorCode, type EffectRef } from "@oxitone/protocol";
import { createAutomationNamespace, Pattern, Project } from "../src/index.js";

const directories: string[] = [];
afterEach(() => {
  for (const path of directories.splice(0)) rmSync(path, { recursive: true, force: true });
});

function rig() {
  const project = new Project({ seed: 71 });
  const bus = project.addMixerChannel({ name: "instrument" });
  const fx = project.addMixerChannel({ name: "return" });
  const channel = project.addChannel({ mixerChannelId: bus.id });
  project
    .addTrack()
    .use(channel)
    .add(
      new Pattern({
        lengthBeats: 1,
        notes: [{ pitch: 60, start: 0, duration: 1, velocity: 0.8 }],
      }),
    )
    .at({ bar: 1 });
  return { project, bus, fx, channel };
}

function samples(path: string): number[] {
  const bytes = readFileSync(path);
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const length = bytes.readUInt32LE(offset + 4);
    if (bytes.toString("ascii", offset, offset + 4) === "data") {
      return Array.from({ length: length / 4 }, (_, index) => bytes.readFloatLE(offset + 8 + index * 4));
    }
    offset += 8 + length + (length % 2);
  }
  throw new Error("WAV missing data chunk");
}

async function render(project: Project): Promise<number[]> {
  const dir = mkdtempSync(join(tmpdir(), "oxitone-mixer-"));
  directories.push(dir);
  const report = await project.renderWav({
    path: join(dir, "mix.wav"),
    end: { seconds: 0.15 },
    tailSeconds: 0,
    bitDepth: "float32",
  });
  return samples(report.files[0]!.path);
}

function scaled(actual: number[], reference: number[], gain: number): void {
  expect(actual).toHaveLength(reference.length);
  let error = 0;
  for (let i = 0; i < actual.length; i++) error = Math.max(error, Math.abs(actual[i]! - reference[i]! * gain));
  expect(error).toBeLessThan(1e-6);
}

const invert = (options: Partial<EffectRef> = {}): EffectRef => ({
  pluginId: "oxitone.utility",
  pluginVersion: "1.0.0",
  parameters: { polarity: 1 },
  ...options,
});

describe("mixer builders through the native WAV facade", () => {
  it("automates insert parameters and bypass on Channel, bus and Master", async () => {
    for (const kind of ["channel", "bus", "master"] as const) {
      const { project, bus, channel } = rig();
      const owner = kind === "channel" ? channel : kind === "bus" ? bus : project.master;
      const reference = await render(project);
      owner.addEffect(invert());
      owner.automate("insert.0.parameter.polarity", createAutomationNamespace().constant(0));
      scaled(await render(project), reference, 1);
      owner.automate("insert.0.bypass", createAutomationNamespace().constant(1));
      scaled(await render(project), reference, 1);
      // Plugin validation belongs to Rust and survives the public TS/native boundary.
      owner.automate("insert.9.parameter.polarity", createAutomationNamespace().constant(0));
      await expect(project.compile()).rejects.toMatchObject({ code: ErrorCode.AutomationTargetInvalid });
    }
  });

  it("applies Channel, bus and Master inserts, including mix and bypass", async () => {
    const { project, bus, channel } = rig();
    const reference = await render(project);
    expect(reference.some((value) => Math.abs(value) > 0.01)).toBe(true);
    channel.addEffect(invert());
    scaled(await render(project), reference, -1);
    bus.addEffect(invert());
    scaled(await render(project), reference, 1);
    project.master.addEffect(invert({ bypass: true }));
    scaled(await render(project), reference, 1);
    project.master.inserts = [invert({ mix: 0.5 })];
    scaled(await render(project), reference, 0);
  });

  it("renders pre/post-fader sends and excludes detector sends from the audible sum", async () => {
    const { project, bus, fx } = rig();
    const reference = await render(project);
    bus.masterSendRatio = 0;
    bus.send(fx, { ratio: 0.5 });
    // The return adds one centered equal-power fader (1/sqrt(2) per side).
    scaled(await render(project), reference, 0.5 * Math.SQRT1_2);
    bus.level = 0;
    scaled(await render(project), reference, 0);
    bus.send(fx, { ratio: 0.5, preFader: true });
    scaled(await render(project), reference, 0.5);
    fx.addEffect({ pluginId: "oxitone.compressor", pluginVersion: "1.0.0", parameters: {} });
    bus.send(fx, { sidechain: true });
    scaled(await render(project), reference, 0);
  });

  it("executes send automation and exports the authored bus stems", async () => {
    const { project, bus, fx } = rig();
    bus.masterSendRatio = 0;
    bus.send(fx);
    const reference = await render(project);
    expect(reference.some((value) => Math.abs(value) > 0.01)).toBe(true);
    bus.automate(`send.${fx.id}.ratio`, createAutomationNamespace().constant(0));
    scaled(await render(project), reference, 0);
    const dir = mkdtempSync(join(tmpdir(), "oxitone-bus-stems-"));
    directories.push(dir);
    const report = await project.renderWav({
      path: dir,
      stems: "mixer-channels",
      end: { seconds: 0.15 },
      tailSeconds: 0,
    });
    expect(
      report.files
        .map((file) => file.stem)
        .filter(Boolean)
        .sort(),
    ).toEqual([bus.id, fx.id].sort());
    expect(report.files.filter((file) => file.stem === undefined)).toHaveLength(1);
    for (const file of report.files) scaled(samples(file.path), reference, 0);
  });

  it("rejects audio and detector cycles at compile with a complete cycle path", async () => {
    for (const sidechain of [false, true]) {
      const { project, bus, fx } = rig();
      bus.send(fx);
      fx.send(bus, { sidechain });
      try {
        const session = await project.compile();
        await session.dispose();
        expect.unreachable("cyclic graph compiled successfully");
      } catch (error) {
        expect(error).toMatchObject({
          code: ErrorCode.InvalidProject,
          message: expect.stringContaining("cycle"),
          details: { path: "$.mixerChannels" },
        });
        expect(String(error)).toContain(bus.id);
        expect(String(error)).toContain(fx.id);
      }
    }
  });

  it("checks plugin parameters and removed send automation against the native graph", async () => {
    const { project, bus, fx, channel } = rig();
    channel.addEffect(invert({ parameters: { unknown: 1 } }));
    await expect(project.compile()).rejects.toMatchObject({ code: ErrorCode.InvalidProject });
    channel.effectChain = [];
    bus.send(fx);
    bus.automate(`send.${fx.id}.ratio`, createAutomationNamespace().constant(0.5));
    bus.removeSend(fx);
    await expect(project.compile()).rejects.toMatchObject({ code: ErrorCode.AutomationTargetInvalid });
  });
});
