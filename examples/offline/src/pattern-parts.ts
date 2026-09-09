import { Pattern, Project, createAutomationNamespace, wavetable } from "@oxitone/core";

const project = new Project({ name: "Pattern study" });
const keys = project.addChannel({ name: "Keys", instrument: wavetable({ oscA: { wave: "triangle" } }) });
const bass = project.addChannel({ name: "Bass", instrument: wavetable({ oscA: { wave: "sine" } }) });
const melody = new Pattern({
  name: "Keys", lengthBeats: 8,
  notes: [60, 64, 67, 71, 69, 67, 64, 62].map((pitch, start) => ({ pitch, start, duration: 0.75, velocity: 0.7 })),
});
const bassline = new Pattern({
  name: "Bass", lengthBeats: 8,
  notes: [36, 36, 41, 43].map((pitch, index) => ({ pitch, start: index * 2, duration: 1.5, velocity: 0.8 })),
});
const verse = new Pattern({
  name: "Verse", lengthBeats: 8,
  parts: [{ channelId: keys.id, pattern: melody }, { channelId: bass.id, pattern: bassline }],
});
const answer = new Pattern({
  name: "Answer", lengthBeats: 8,
  parts: [{ channelId: keys.id, pattern: melody.transpose(7) }, { channelId: bass.id, pattern: bassline }],
});
const arrangement = project.addTrack("Patterns");
arrangement.add(verse).at({ bar: 1 });
arrangement.add(answer).at({ bar: 3 });
arrangement.add(verse).at({ bar: 5 });
const motion = createAutomationNamespace().line(0.2, 0.5, 8);
const lane = project.addAutomationLane({ entityId: bass.id, parameterId: "level" }, motion, { playback: "playlist" });
const automation = project.addTrack("Bass level");
project.createAutomationClip(lane, automation, 4, 8);
project.createAutomationClip(lane, automation, 20, 4);

export default project;
