import { describe, expect, it } from "vitest";
import { ErrorCode, type EffectRef } from "@oxitone/protocol";
import { createAutomationNamespace, Project } from "../src/index.js";

const utility = (): EffectRef => ({
  pluginId: "oxitone.utility",
  pluginVersion: "1.0.0",
  parameters: { polarity: 1 },
  mix: 0.5,
});

describe("mixer and insert authoring", () => {
  it("owns an editable Master and serializes sorted buses with channel routing", () => {
    const project = new Project();
    expect(project.revision).toBe(0);
    expect(project.master.id).toBe(project.masterMixerChannelId);
    expect(project.master.masterSendRatio).toBeUndefined();
    const bus = project.addMixerChannel({ name: "keys", level: 0.8, balance: -0.2 });
    const channel = project.addChannel({ mixerChannelId: bus.id, swing: 0.2, mute: true, solo: true });
    project.master.addEffect(utility());
    project.master.level = 0.75;
    const snapshot = project.snapshot();
    expect(snapshot.mixerChannels.map((entry) => entry.id)).toEqual([project.master.id, bus.id].sort());
    expect(snapshot.mixerChannels.find((entry) => entry.id === project.master.id)).toMatchObject({
      level: 0.75,
      inserts: [utility()],
      sends: [],
    });
    expect(snapshot.channels[0]).toMatchObject({ mixerChannelId: bus.id, swing: 0.2, mute: true, solo: true });
    channel.mixerChannelId = project.master.id;
    expect(channel.toSpec().mixerChannelId).toBe(project.master.id);
    expect(snapshot.channels[0]?.mixerChannelId).toBe(bus.id);
  });

  it("detaches constructor values, getters, snapshots and insert replacements", () => {
    const project = new Project();
    const effect = utility();
    const instrument = { pluginId: "oxitone.wavetable", pluginVersion: "1.0.0", parameters: { level: 0.5 } };
    const bus = project.addMixerChannel({ inserts: [effect] });
    const channel = project.addChannel({ instrument, effectChain: [effect] });
    const original = project.snapshot();
    const revision = project.revision;
    effect.parameters.polarity = 0;
    instrument.parameters.level = 0;
    channel.instrument.parameters.level = 0;
    channel.effectChain[0]!.parameters.polarity = 0;
    bus.inserts[0]!.parameters.polarity = 0;
    project.snapshot().mixerChannels[0]!.inserts.length = 0;
    expect(project.snapshot()).toEqual(original);
    expect(project.revision).toBe(revision);
    channel.effectChain = [effect];
    bus.inserts = [];
    effect.parameters.polarity = 1;
    expect(channel.effectChain[0]?.parameters.polarity).toBe(0);
    expect(bus.inserts).toEqual([]);
    expect(project.revision).toBe(revision + 2);
  });

  it("upserts sends without duplicate destinations and binds bus automation", () => {
    const project = new Project();
    const bus = project.addMixerChannel();
    const fx = project.addMixerChannel();
    bus.masterSendRatio = 0;
    bus.send(fx, { ratio: 0.2, preFader: true });
    bus.send(fx, { ratio: 0.8, sidechain: true });
    const revision = project.revision;
    expect(bus.sends).toEqual([{ destinationId: fx.id, ratio: 0.8, sidechain: true }]);
    bus.sends[0]!.ratio = 0;
    expect(bus.sends[0]?.ratio).toBe(0.8);
    expect(project.revision).toBe(revision);
    const parameter = `send.${fx.id}.ratio`;
    const lane = bus.automate(parameter, createAutomationNamespace().constant(0.4));
    expect(project.snapshot().automation[0]?.target).toEqual({ entityId: bus.id, parameterId: parameter });
    expect(lane.target.entityId).toBe(bus.id);
    bus.removeSend(fx);
    expect(bus.sends).toEqual([]);
  });

  it("rejects invalid changes atomically with stable errors", () => {
    const project = new Project();
    const bus = project.addMixerChannel();
    const fx = project.addMixerChannel();
    const channel = project.addChannel();
    const foreign = new Project().addMixerChannel(); // Same seed/ID still belongs to another project.
    const mutations = [
      () => {
        bus.level = Number.NaN;
      },
      () => {
        bus.balance = 1.1;
      },
      () => {
        bus.masterSendRatio = 2;
      },
      () => {
        project.master.masterSendRatio = 0;
      },
      () => {
        channel.swing = -1;
      },
      () => {
        channel.pan = Number.POSITIVE_INFINITY;
      },
      () => {
        channel.mixerChannelId = "mix_missing";
      },
      () => channel.addEffect({ ...utility(), mix: 1.1 }),
      () => bus.addEffect({ ...utility(), parameters: { gainDb: Number.NaN } }),
      () => bus.send(fx, { ratio: -1 }),
      () => bus.send(project.master),
      () => project.master.send(bus),
      () => bus.send(foreign),
      () => bus.removeSend(foreign),
      () => project.addMixerChannel({ level: -1 }),
      () => project.addChannel({ mixerChannelId: "mix_missing" }),
    ];
    for (const mutate of mutations) {
      const before = project.snapshot();
      expect(mutate).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
      expect(project.snapshot()).toEqual(before);
    }
  });

  it("increments revision once for each accepted parameter/chain/routing mutation", () => {
    const project = new Project();
    const bus = project.addMixerChannel();
    const channel = project.addChannel();
    const mutations = [
      () => {
        bus.level = 0.5;
      },
      () => {
        bus.balance = 0.2;
      },
      () => {
        bus.mute = true;
      },
      () => {
        bus.solo = true;
      },
      () => {
        bus.masterSendRatio = 0.25;
      },
      () => bus.addEffect(utility()),
      () => {
        channel.level = 0.4;
      },
      () => {
        channel.pan = -0.4;
      },
      () => {
        channel.swing = 0.5;
      },
      () => {
        channel.mute = true;
      },
      () => {
        channel.solo = true;
      },
      () => channel.addEffect(utility()),
      () => {
        channel.mixerChannelId = bus.id;
      },
    ];
    for (const mutate of mutations) {
      const revision = project.revision;
      mutate();
      expect(project.revision).toBe(revision + 1);
      expect(project.snapshot().revision).toBe(String(revision + 1));
    }
  });
});
