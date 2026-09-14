import { it, expect } from "vitest";
import { Project, Pattern, wavetable, createEngine, dispose, getProtocolVersion } from "../src/index.js";

it("provides authoring and native execution through the single oxitone entry", async () => {
  const project = new Project({ seed: 7 });
  project
    .addTrack()
    .use(project.addChannel({ instrument: wavetable() }))
    .add(
      new Pattern({
        lengthBeats: 1,
        notes: [{ pitch: 60, start: 0, duration: 0.5, velocity: 0.6 }],
      }),
    )
    .at({ bar: 1 });
  const session = await project.compile();
  try {
    expect((await session.exportMidi({})).diagnostics.noteTrackCount).toBe(1);
  } finally {
    await session.dispose();
  }
  const engine = createEngine();
  try {
    expect(engine.protocolVersion).toBe(getProtocolVersion());
  } finally {
    dispose(engine);
  }
});
