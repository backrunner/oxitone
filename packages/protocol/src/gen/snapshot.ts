import { beatToWire, encodeProjectSnapshot, projectSnapshotSchema } from "../index.js";
import type { ProjectSnapshot } from "../index.js";
import { write } from "./output.js";

/** Canonical full-snapshot fixture; the Rust byte round-trip test target. */
export function generateSnapshotFixture(): void {
  const snapshot: ProjectSnapshot = projectSnapshotSchema.parse({
    protocolVersion: "1.0",
    revision: "7",
    id: "prj_0001",
    name: "Canonical Demo",
    sampleRate: 48000,
    blockSize: 128,
    seed: 99,
    tempoMap: [
      { startBeat: beatToWire(0), bpm: 120, curve: "step" },
      { startBeat: beatToWire(16), bpm: 140, curve: "linear" },
    ],
    timeSignatureMap: [
      { startBar: 1, numerator: 4, denominator: 4 },
      { startBar: 9, numerator: 3, denominator: 4 },
    ],
    markers: [
      { id: "mrk_0001", name: "intro", startBeat: beatToWire(0) },
      { id: "mrk_0002", name: "chorus", startBeat: beatToWire(16) },
    ],
    tracks: [
      {
        id: "trk_0001",
        name: "drums",
        channelIds: ["chn_0001"],
        patternClipIds: ["pcl_0001"],
        sampleClipIds: [],
        midiChannel: 10,
      },
      {
        id: "trk_0002",
        name: "bass",
        channelIds: ["chn_0002"],
        patternClipIds: [],
        sampleClipIds: ["scl_0001"],
      },
    ],
    patterns: [
      {
        id: "pat_0001",
        name: "four-floor",
        lengthBeats: beatToWire(4),
        notes: [
          { id: "not_0001", pitch: 36, start: beatToWire(0), duration: beatToWire(0.25), velocity: 0.9 },
          {
            id: "not_0002",
            pitch: 38,
            start: beatToWire(1),
            duration: beatToWire(0.5),
            velocity: 0.8,
            chance: 0.9,
          },
          {
            id: "not_0003",
            pitch: 42,
            start: beatToWire(0.5),
            duration: beatToWire(0.25),
            velocity: 0.6,
            tags: ["hh"],
          },
        ],
      },
    ],
    patternClips: [
      {
        id: "pcl_0001",
        patternId: "pat_0001",
        trackId: "trk_0001",
        startBeat: beatToWire(0),
        loopCount: 8,
      },
    ],
    sampleClips: [
      {
        id: "scl_0001",
        sampleId: "smp_0001",
        trackId: "trk_0002",
        startBeat: beatToWire(0),
        durationBeats: beatToWire(16),
        gain: 1,
        loop: { lengthBeats: beatToWire(4), count: 4 },
        tempoSync: "stretch",
        stretchAlgorithm: "wsola-v1",
      },
    ],
    samples: [
      {
        id: "smp_0001",
        assetUri: "assets/bass-loop.wav",
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        format: "wav",
        sampleRate: 48000,
        channels: 2,
        frames: "192000",
        edits: {
          startFrame: "0",
          endFrame: "190000",
          level: 1,
          normalize: { peakDb: -1 },
          fadeIn: { lengthFrames: "256", curve: "equalPower" },
        },
        musicalLengthBeats: beatToWire(4),
      },
    ],
    channels: [
      {
        id: "chn_0001",
        name: "drum-bus",
        instrument: {
          pluginId: "oxitone.sampler",
          pluginVersion: "1.0.0",
          parameters: { rootKey: 60, gainDb: 0 },
          resources: { sample: "smp_0001" },
        },
        effectChain: [
          {
            pluginId: "oxitone.eq",
            pluginVersion: "1.0.0",
            parameters: { band1GainDb: 1.5 },
            mix: 1,
          },
        ],
        level: 1,
        pan: 0,
        mixerChannelId: "mix_0001",
      },
      {
        id: "chn_0002",
        name: "bass",
        instrument: {
          pluginId: "oxitone.wavetable",
          pluginVersion: "1.0.0",
          parameters: { cutoff: 0.65, resonance: 0.2 },
        },
        effectChain: [],
        level: 0.8,
        pan: -0.2,
        swing: 0.1,
        mixerChannelId: "mix_0001",
      },
    ],
    mixerChannels: [
      {
        id: "mix_0001",
        name: "bus",
        level: 1,
        balance: 0,
        masterSendRatio: 1,
        inserts: [
          {
            pluginId: "oxitone.compressor",
            pluginVersion: "1.0.0",
            parameters: { thresholdDb: -12, ratio: 4 },
            bypass: false,
          },
        ],
        sends: [{ destinationId: "mix_0002", ratio: 0.25, preFader: false, sidechain: true }],
      },
      {
        id: "mix_0002",
        name: "fx",
        level: 0.9,
        balance: 0,
        inserts: [
          {
            pluginId: "oxitone.reverb",
            pluginVersion: "1.0.0",
            parameters: { decaySeconds: 1.8, mix: 1 },
          },
        ],
        sends: [],
      },
    ],
    automation: [
      {
        id: "auto_0001",
        target: { entityId: "prj_0001", parameterId: "tempo" },
        source: {
          kind: "curve",
          interpolation: "linear",
          points: [
            { beat: beatToWire(0), value: 0.25 },
            { beat: beatToWire(32), value: 0.5 },
          ],
        },
        lastBeat: beatToWire(64),
      },
      {
        id: "auto_0002",
        target: { entityId: "chn_0001", parameterId: "level" },
        source: {
          kind: "wave",
          wave: "sine",
          periodBeats: beatToWire(8),
          min: 0.2,
          max: 0.9,
        },
      },
      {
        id: "auto_0003",
        target: { entityId: "chn_0002", parameterId: "pan" },
        source: {
          kind: "chance",
          rate: 2,
          probability: 0.72,
          seed: 17,
          smoothBeats: beatToWire(0.04),
          randomPhase: "absolute",
        },
      },
      {
        id: "auto_0004",
        target: { entityId: "mix_0002", parameterId: "mute" },
        source: { kind: "gate", periodBeats: beatToWire(0.5), duty: 0.5 },
        loop: { lengthBeats: beatToWire(8), count: 2 },
      },
    ],
  });
  write("schemas/fixtures/project-snapshot.canonical.json", encodeProjectSnapshot(snapshot));
}
