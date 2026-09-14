import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { Project } from "@oxitone/core";
import { encodeProjectSnapshot } from "@oxitone/protocol";
import { createSong } from "./song.js";
import { createChops } from "./chops.js";

const output = resolve(process.argv[2] ?? fileURLToPath(new URL("../../../target/examples/offline", import.meta.url)));
await mkdir(output, { recursive: true });
const project = createSong();
const source = join(output, "phrase.wav");
const phrase = await project.renderWav({ path: source, tailSeconds: 0 });
await project.exportMidi({ path: join(output, "phrase.mid") });
await writeFile(join(output, "phrase.snapshot.json"), encodeProjectSnapshot(project.snapshot()));
await project.save(join(output, "phrase-project"));

const chops = createChops(source, output);
await chops.save(join(output, "chops-project"), { assetBaseDir: output });
// Loading retains the asset directory for later compile, render and save calls.
const restored = await Project.load(join(output, "chops-project"));
const first = join(output, "chops.wav");
const second = join(output, "chops-restored.wav");
const render = await chops.renderWav({ path: first, assetBaseDir: output, tailSeconds: 0 });
await restored.renderWav({ path: second, tailSeconds: 0 });
const hash = async (path: string) =>
  createHash("sha256")
    .update(await readFile(path))
    .digest("hex");
const firstHash = await hash(first);
if (firstHash !== (await hash(second))) throw new Error("Restored project rendered different audio");
if (!phrase.files.some((file) => file.peakDbfs > -60) || !render.files.some((file) => file.peakDbfs > -60)) {
  throw new Error("Expected audible example output");
}
const report = JSON.stringify(
  { output, phrase: phrase.files, chops: render.files, restoredSha256: firstHash },
  null,
  2,
);
await writeFile(join(output, "report.json"), `${report}\n`);
console.log(report);
