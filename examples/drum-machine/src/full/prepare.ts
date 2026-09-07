import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildPlugins } from "../plugins.js";
import { outputRoot } from "./paths.js";
import { preparePianos } from "./piano.js";

await mkdir(outputRoot, { recursive: true });
const [drums] = await buildPlugins(join(outputRoot, "plugins"));
await writeFile(join(outputRoot, "drums.json"), `${JSON.stringify(drums, null, 2)}\n`);
await preparePianos();
console.log(`Electronic drums and recorded soft/grand piano ready: ${outputRoot}`);
