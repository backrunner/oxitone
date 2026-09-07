import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { buildPlugins } from "../plugins.js";
import { outputRoot, preparePiano } from "./piano.js";

await preparePiano();
const [drums] = buildPlugins(join(outputRoot, "plugins"));
await writeFile(join(outputRoot, "drums.json"), `${JSON.stringify(drums, null, 2)}\n`);
console.log(`Piano and native drums ready: ${outputRoot}`);
