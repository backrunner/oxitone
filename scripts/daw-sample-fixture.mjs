import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

/** A short offline tone; the native capture always uses the simulated sink. */
export async function dawSampleFixture(root, entry) {
  await mkdir(join(root, "Samples"));
  const wav = Buffer.alloc(44 + 9600);
  wav.write("RIFF");
  wav.writeUInt32LE(wav.length - 8, 4);
  wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(48000, 24);
  wav.writeUInt32LE(96000, 28);
  wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34);
  wav.write("data", 36);
  wav.writeUInt32LE(9600, 40);
  for (let i = 0; i < 4800; i++) wav.writeInt16LE(Math.round(Math.sin(i / 30) * 4000), 44 + i * 2);
  await writeFile(join(root, "Samples/kick.wav"), wav);
  const sample = {
    id: "smp_fixture",
    assetUri: "Samples/kick.wav",
    sha256: createHash("sha256").update(wav).digest("hex"),
    format: "wav",
    channels: 1,
    sampleRate: 48000,
    frames: "4800",
  };
  await writeFile(
    entry,
    (await readFile(entry, "utf8")).replace(
      "export default project;",
      `const audio = project.importSampleRef(${JSON.stringify(sample)});\nproject.addChannel({ name: 'Sample kit', instrument: { pluginId: 'oxitone.sampler', pluginVersion: '1.0.0', parameters: {}, resources: { sample: audio.id } } });\nexport default project;`,
    ),
  );
}
