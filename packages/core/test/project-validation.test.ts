import { expect, it } from "vitest";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";
import { Project } from "../src/index.js";

it("reports stable authoring errors before mutating a configuration or arrangement", () => {
  const project = new Project();
  project.addChannel();
  project.addTrack();
  const before = project.snapshot();
  const invalid = [
    () => project.configure({ kind: "channel", index: 0, values: { level: 0.5, pan: 2 } }),
    () => project.configure({ kind: "track", index: 0 }),
    () => project.arrange({ kind: "pattern", action: "place", resource: 0, track: 0, startBeat: -1 }),
  ];
  for (const call of invalid) {
    expect(call).toThrow(OxitoneError);
    expect(call).toThrowError(expect.objectContaining({ code: ErrorCode.InvalidProject }));
    expect(project.snapshot()).toEqual(before);
  }
});
