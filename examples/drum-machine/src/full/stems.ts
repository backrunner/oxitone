import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { createEngine, dispose, registerPlugin, renderWav } from "oxitone";
import { createDubstepSong } from "./melodic-dubstep.js";
import { drumRegistration } from "./shared.js";
import { outputRoot } from "./paths.js";
import { inspectSections } from "./verify.js";

/** Direct master-contribution taps preserve actual sidechain/ducking and expose bass alone. */
const path = join(outputRoot, "drop-stems");
await mkdir(path, { recursive: true });
const engine = createEngine({ allowPlugins: "any" });
try {
  registerPlugin(engine, drumRegistration());
  const snapshot = createDubstepSong().snapshot();
  const result = renderWav(engine, snapshot, { path, start: { bar: 25 }, end: { bar: 33 },
    tailSeconds: 0, bitDepth: 24, dither: "none", stems: "mixer-channels" });
  const stems = result.files.map(file => ({ ...file,
    name: snapshot.mixerChannels.find(t => t.id === file.stem)?.name ?? "Master",
    level: inspectSections(file.path, [["Drop I · first statement", 0]], 140, 8)[0]!,
  }));
  for (const name of ["Sub · short kick duck", "Bassline · harmonic foundation", "Mid bass · kick sidechain"]) {
    const stem = stems.find(s => s.name === name);
    assert(stem && stem.level.rmsDbfs > -42, `${name} is missing or too quiet before master processing`);
  }
  for (const [name, floor] of [["Kick · detector", -28], ["Snare · body / crack / tail", -27],
    ["Tops · hats / ride / shaker", -38]] as const) {
    assert(stems.find(s => s.name === name)!.level.rmsDbfs > floor, `${name} lost its independent drum energy`);
  }
  await writeFile(join(path, "report.json"), `${JSON.stringify({ ...result, files: stems }, null, 2)}\n`);
  console.log(stems.filter(s => /bass|Sub|Music|Master|Kick|Snare|Tops/i.test(s.name)).map(s => ({
    name: s.name, rmsDbfs: s.level.rmsDbfs, bandRmsDbfs: s.level.bandRmsDbfs,
  })));
} finally { dispose(engine); }
