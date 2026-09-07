import { build } from "esbuild";
import { copyFile, mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
export const options = {
  absWorkingDir: fileURLToPath(new URL("../../../", import.meta.url)), entryPoints: {
    main: "examples/web/src/main.ts", song: process.env.OXITONE_WEB_SONG_ENTRY ?? "examples/web/src/song.ts",
    worker: "packages/web/src/worker.ts", worklet: "packages/web/src/worklet.ts",
  }, bundle: true, format: "esm", platform: "browser", target: "es2022", outdir: "examples/web/dist", sourcemap: true,
};
export async function bundle() {
  await mkdir(new URL("../dist/", import.meta.url),{recursive:true});
  await build(options);
  await copyFile(new URL("../index.html",import.meta.url),new URL("../dist/index.html",import.meta.url));
}
if (process.argv[1] === fileURLToPath(import.meta.url)) await bundle();
