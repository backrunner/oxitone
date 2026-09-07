import { bundle, options } from "./build.mjs";
import { context } from "esbuild";
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../../",import.meta.url)), assets = new Map();
await bundle();
const clients = new Set();
const watcher = await context({ ...options, plugins: [{ name: "project-watch", setup(build) {
  build.onEnd(result => {
    const message = JSON.stringify({ ok: result.errors.length === 0, revision: Date.now(),
      message: result.errors.map(error => error.text).join("\n") });
    for (const client of clients) client.write(`data: ${message}\n\n`);
  });
} }] });
await watcher.watch();
const songs = ["after-the-horizon"];
for(const slug of songs) {
  try {
    const snapshot=JSON.parse(await readFile(resolve(root,"target/examples/full-songs",slug+".snapshot.json"),"utf8"));
    for(const sample of snapshot.samples) assets.set(sample.sha256,resolve(root,sample.assetUri));
  } catch { /* Optional full songs are produced by example:songs first. */ }
}
const types={".html":"text/html",".js":"text/javascript",".map":"application/json",".json":"application/json",".wasm":"application/wasm",".wav":"audio/wav"};
const server=createServer(async(req,res)=>{
  res.setHeader("Cross-Origin-Opener-Policy","same-origin");
  res.setHeader("Cross-Origin-Embedder-Policy","require-corp");
  res.setHeader("Cache-Control","no-store");
  try {
    const path=new URL(req.url,"http://localhost").pathname;
    if (path === "/events") {
      res.setHeader("Content-Type", "text/event-stream");
      res.write(": Oxitone project watch\n\n"); clients.add(res);
      req.on("close", () => clients.delete(res)); return;
    }
    let file;
    if(path==="/oxitone.wasm") file=resolve(root,"packages/web/dist/oxitone.wasm");
    else if(path.startsWith("/assets/")) file=assets.get(path.slice(8));
    else if(songs.some(s=>path===`/songs/${s}.snapshot.json`)) file=resolve(root,"target/examples/full-songs",path.split("/").pop());
    else {
      const base=resolve(root,"examples/web/dist");
      file=resolve(base,"."+decodeURIComponent(path==="/" ? "/index.html" : path));
      if(!file.startsWith(base+sep)) throw new Error("outside web root");
    }
    if(!file || !(await stat(file)).isFile()) throw new Error("missing");
    res.setHeader("Content-Type",types[extname(file)] ?? "application/octet-stream");
    res.end(await readFile(file));
  } catch { res.writeHead(404);res.end("Not found; run pnpm build:wasm and pnpm example:songs for optional songs."); }
});
server.listen(Number(process.env.OXITONE_WEB_PORT ?? 4173),"127.0.0.1",()=>{
  console.log("Oxitone Web Audio: http://127.0.0.1:"+server.address().port);
});
