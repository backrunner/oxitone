import { effect, softPiano, grandPiano, type PianoBank, type Project } from "@oxitone/core";
import { electronicKit } from "../electronic-kit.js";
import { fx } from "./shared.js";
import { crystalKeys, skyChords, horizonLead, skyPad, tapeGlass, chordBody, subBass,
  noiseFx, downlifter, chordPluck, reeseBass, halo } from "./synth-patches.js";
import { foundationBass, turbineBass, talkingBass, laserBass, snareClap } from "./bass-patches.js";

const hp = (hz: number) => fx("filter", { mode: 1, cutoffHz: hz, resonance: 0.707 });
const lp = (hz: number) => fx("filter", { mode: 0, cutoffHz: hz, resonance: 0.707 });
const eq = (lowMidDb: number, presenceDb = 0) => fx("eq", {
  "band2.freqHz": 340, "band2.q": 0.8, "band2.gainDb": lowMidDb,
  "band3.freqHz": 3200, "band3.q": 1, "band3.gainDb": presenceDb,
});

/** Explicit low-end separation, return filtering, per-track inserts and bus headroom. */
export function createMix(p: Project, bank: PianoBank) {
  const music = p.addMixerChannel({ name: "Music · rhythmic duck", inserts: [eq(-1.2, -0.6)] });
  const hall = p.addMixerChannel({ name: "Sky hall · ducked return", inserts: [
    effect("reverb", { decaySeconds: 2.1, damping: 0.62, predelayMs: 28,
      highpassHz: 520, lowpassHz: 6800, ducking: 0.4, width: 1.15 }), hp(420)] });
  const echo = p.addMixerChannel({ name: "Ping-pong · ducked return", inserts: [hp(500),
    fx("delay", { timeBeats: 0.75, feedback: 0.32, pingPong: 1, feedbackFilterHz: 4800, highpassHz: 420, ducking: 0.45 }), lp(6500)] });
  echo.send(hall, { ratio: 0.12 });
  const room = p.addMixerChannel({ name: "Drum room · early reflections", inserts: [
    effect("convolver", { predelayMs: 8, highpassHz: 650, lowpassHz: 6500, outputDb: -3 }), hp(500)] });
  const keysBus = p.addMixerChannel({ name: "Piano / plucks", masterSendRatio: 0, inserts: [
    effect("tape", { driveDb: 1.5, outputDb: -1.5, wow: 0.015, flutter: 0.01, toneHz: 11000 }, { mix: 0.3 })] });
  keysBus.send(music, { ratio: 1 });
  keysBus.send(hall, { ratio: 0.2 }); keysBus.send(echo, { ratio: 0.12 });
  const kickBus = p.addMixerChannel({ name: "Kick · detector", inserts: [hp(28), eq(-3, 1.5)] });
  const drumsBus = p.addMixerChannel({ name: "Snare / tops", inserts: [hp(130),
    effect("compactor", { thresholdDb: -26, upwardDb: 2, transient: 0.15, attackMs: 12, releaseMs: 80 }, { mix: 0.4 }),
    fx("compressor", { thresholdDb: -16, ratio: 2, attackMs: 18, releaseMs: 85, kneeDb: 5 }),
    fx("distortion", { driveDb: 3, outputDb: -3 }, 0.14)] });
  drumsBus.send(room, { ratio: 0.13 }); drumsBus.send(hall, { ratio: 0.035 });
  const synthBus = p.addMixerChannel({ name: "Supersaw stack", masterSendRatio: 0,
    inserts: [hp(170), eq(-1.2, -0.8),
      effect("multibandDynamics", { depth: 0.24, upwardDb: 5, downwardRatio: 2.2, time: 1.2, highGainDb: -1 }),
      effect("spreader", { width: 1.12, amount: 0.1, bassMonoHz: 260 })] });
  synthBus.send(music, { ratio: 1 }); synthBus.send(hall, { ratio: 0.12 });
  const subBus = p.addMixerChannel({ name: "Sub · short kick duck", inserts: [hp(23), lp(110)] });
  const lowBus = p.addMixerChannel({ name: "Bassline · harmonic foundation", inserts: [hp(85), lp(850)] });
  const bassBus = p.addMixerChannel({ name: "Mid bass · kick sidechain", inserts: [
    fx("compressor", { thresholdDb: -16, ratio: 2.5, attackMs: 0.2, releaseMs: 55, kneeDb: 4 }),
    effect("spreader", { width: 0.85, amount: 0, bassMonoHz: 200 })] });
  kickBus.send(bassBus, { ratio: 1, sidechain: true });
  const leadBus = p.addMixerChannel({ name: "Hook / answers", masterSendRatio: 0, inserts: [hp(320), eq(-1.5, -1)] });
  leadBus.send(music, { ratio: 1 });
  leadBus.send(echo, { ratio: 0.14 }); leadBus.send(hall, { ratio: 0.09 });
  const throws = p.addMixerChannel({ name: "Hook · phrase delay throws", inserts: [hp(680),
    effect("delay", { timeBeats: 0.75, feedback: 0.4, pingPong: 1, feedbackFilterHz: 4300, highpassHz: 650, ducking: 0.55 })] });
  leadBus.send(throws, { ratio: 0.22 });
  const keys = p.addChannel({ name: "Soft Piano · intimate intro", instrument: softPiano(bank), mixerChannelId: keysBus.id, level: 1.15, effectChain: [hp(120), lp(9000)] });
  const grand = p.addChannel({ name: "Grand Piano · open breakdown", instrument: grandPiano(bank), mixerChannelId: keysBus.id, level: 0.75, effectChain: [hp(160)] });
  const kick = p.addChannel({ name: "Circuit · tuned punch kick", instrument: electronicKit(), mixerChannelId: kickBus.id, level: 1 });
  const drums = p.addChannel({ name: "Circuit · body / snap snare", instrument: electronicKit(), mixerChannelId: drumsBus.id, level: 1.12 });
  const tops = p.addChannel({ name: "Circuit · metallic hats", instrument: electronicKit(), mixerChannelId: drumsBus.id, level: 0.45, effectChain: [hp(3500)] });
  const clap = p.addChannel({ name: "Clap · snare width", instrument: snareClap(), mixerChannelId: drumsBus.id, level: 0.3, effectChain: [hp(1100)] });
  const saw = p.addChannel({ name: "Sky · 7 + 3 supersaw", instrument: skyChords(), mixerChannelId: synthBus.id, level: 1.5,
    effectChain: [fx("distortion", { driveDb: 3, outputDb: -2, toneHz: 14000 }, 0.12)] });
  const body = p.addChannel({ name: "Sky · center chord body", instrument: chordBody(), mixerChannelId: synthBus.id, level: 0.34, effectChain: [hp(210), lp(3600)] });
  const sub = p.addChannel({ name: "Foundation · pure mono sub", instrument: subBass(), mixerChannelId: subBus.id, level: 0.5 });
  const foundation = p.addChannel({ name: "Foundation · harmonic bassline", instrument: foundationBass(), mixerChannelId: lowBus.id, level: 0.62,
    effectChain: [fx("distortion", { driveDb: 6, outputDb: -5, toneHz: 1400 }, 0.45)] });
  const growl = p.addChannel({ name: "Signal · FM turbine", instrument: turbineBass(), mixerChannelId: bassBus.id, level: 0.95,
    effectChain: [hp(130), fx("distortion", { mode: 0, driveDb: 13, outputDb: -8, toneHz: 6200 }, 0.72),
      effect("multibandDynamics", { depth: 0.4, upwardDb: 9, downwardRatio: 3, time: 0.7, highGainDb: -2, outputDb: -1 }),
      effect("nonlinearFilter", { cutoffHz: 5200, driveDb: 2, outputDb: -2, resonance: 0.13 }), hp(155)] });
  const vowel = p.addChannel({ name: "Formant · talking bass", instrument: talkingBass(), mixerChannelId: bassBus.id, level: 0.85,
    effectChain: [hp(180), fx("distortion", { mode: 3, driveDb: 8, outputDb: -7, toneHz: 5000 }, 0.5),
      effect("multibandDynamics", { depth: 0.3, upwardDb: 7, downwardRatio: 2.5 })] });
  const laser = p.addChannel({ name: "Laser · sync stab", instrument: laserBass(), mixerChannelId: bassBus.id, level: 0.52,
    effectChain: [hp(180), fx("distortion", { mode: 2, driveDb: 9, outputDb: -7, toneHz: 5400 }, 0.5)] });
  const lead = p.addChannel({ name: "Horizon · center lead", instrument: horizonLead(), mixerChannelId: leadBus.id, level: 0.94,
    effectChain: [fx("distortion", { driveDb: 4, outputDb: -3 }, 0.18)] });
  const air = p.addChannel({ name: "Bloom · moving pad", instrument: skyPad(), mixerChannelId: music.id, level: 0.16,
    effectChain: [hp(480), effect("flanger", { rateHz: 0.07, depthMs: 1.2, feedback: 0.18, stereo: 0.7 }, { mix: 0.12 })] });
  const sparkle = p.addChannel({ name: "Prism · countermelody", instrument: tapeGlass(), mixerChannelId: leadBus.id, pan: -0.2, level: 0.23 });
  const arp = p.addChannel({ name: "Orbit · stereo pluck arp", instrument: crystalKeys(), mixerChannelId: leadBus.id, pan: 0.2, level: 0.18, effectChain: [hp(600)] });
  const lift = p.addChannel({ name: "Air · filtered build", instrument: noiseFx(), mixerChannelId: music.id, level: 0.19, effectChain: [hp(1100)] });
  const fall = p.addChannel({ name: "Air · downlifter", instrument: downlifter(), mixerChannelId: music.id, level: 0.2, effectChain: [hp(1800), lp(10000)] });
  const pluck = p.addChannel({ name: "Ember · chord pluck", instrument: chordPluck(), mixerChannelId: keysBus.id,
    level: 0.26, pan: -0.12, effectChain: [hp(300)] });
  const reese = p.addChannel({ name: "Undertow · rounded Reese", instrument: reeseBass(), mixerChannelId: bassBus.id,
    level: 0.34, effectChain: [hp(150), effect("tape", { driveDb: 4, outputDb: -4, toneHz: 4800, wow: 0, flutter: 0 })] });
  const shimmer = p.addChannel({ name: "Halo · upper harmonics", instrument: halo(), mixerChannelId: synthBus.id,
    level: 0.16, effectChain: [hp(1600)] });
  const impact = p.addChannel({ name: "Circuit · impact tail", instrument: { ...electronicKit(), parameters: { ...electronicKit().parameters, openDecay: 2, hatTone: 0.2 } },
    mixerChannelId: hall.id, level: 0.6, effectChain: [hp(2300)] });
  p.master.inserts = [hp(23), eq(-0.5, -0.5),
    effect("compressor", { thresholdDb: -15, ratio: 1.4, attackMs: 30, releaseMs: 140, kneeDb: 6, sidechainHighpassHz: 120 }),
    effect("spreader", { width: 1, amount: 0, bassMonoHz: 120 }),
    effect("limiter", { inputDb: 16, ceilingDb: -1.2, releaseMs: 80 })];
  return { keys, kick, drums, tops, saw, body, sub, growl, vowel, lead, air, sparkle, arp, lift, fall, impact,
    pluck, reese, shimmer, grand, foundation, laser, clap, subBus, lowBus, music, hall, echo, throws, leadBus };
}
