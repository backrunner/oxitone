import { createAutomationNamespace } from "@oxitone/core";
import createProject from "./preview.js";

/** A routed version of Midnight Circuit for exploring the read-only Mixer. */
export default function createMixerPreview() {
  const project = createProject();
  const drums = project.addMixerChannel({ name: "Drum bus" });
  const music = project.addMixerChannel({
    name: "Music bus",
    masterSendRatio: 0.82,
    inserts: [{ pluginId: "oxitone.compressor", pluginVersion: "1.0.0", parameters: {} }],
  });
  const room = project.addMixerChannel({
    name: "Room reverb",
    level: 0.7,
    inserts: [{ pluginId: "oxitone.reverb", pluginVersion: "1.0.0", parameters: {}, mix: 1 }],
  });
  const echo = project.addMixerChannel({
    name: "Tape echo",
    masterSendRatio: 0.65,
    inserts: [
      { pluginId: "oxitone.delay", pluginVersion: "1.0.0", parameters: { timeBeats: 0.75, feedback: 0.3 }, mix: 1 },
    ],
  });
  for (const [index, channel] of project.channels.entries()) channel.mixerChannelId = index === 0 ? drums.id : music.id;
  drums.send(room, { ratio: 0.18 });
  drums.send(music, { ratio: 0.65, sidechain: true });
  music.send(room, { ratio: 0.24 });
  music.send(echo, { ratio: 0.16, preFader: true });
  music.automate(`send.${room.id}.ratio`, createAutomationNamespace().sine({ periodBeats: 16, min: 0.18, max: 0.3 }));
  return project;
}
