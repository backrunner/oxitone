import { Pattern, Project, slicer } from "@oxitone/core";
import { importSample } from "@oxitone/samples";

/** Reuse the rendered phrase as a portable sample and slice it at a faster tempo. */
export function createChops(source: string, directory: string): Project {
  const project = new Project({ name: "Chopped phrase", seed: 43 });
  const imported = importSample(source, { assetBaseDir: directory, cacheDir: "cache" });
  const sample = project.addSample({ ...imported, musicalLengthBeats: 8 });
  const channel = project.addChannel({ name: "Slices", level: 0.7,
    instrument: slicer(sample, { slices: { grid: 8 }, tempoSync: "repitch" }),
  });
  project.setTempo(150, "linear");
  project.addTempoSegment({ startBeat: 8, bpm: 180 });
  const order = [0, 2, 1, 3, 4, 6, 5, 7];
  project.addTrack("Slices").use(channel).add(new Pattern({ lengthBeats: 8,
    notes: order.map((slice, i) => ({ pitch: 60 + slice, start: i, duration: 0.9, velocity: 1 })),
  })).at({ bar: 1 });
  return project;
}
