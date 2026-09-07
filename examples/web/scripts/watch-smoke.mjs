import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { launchSilentBrowser, silentPage } from "./silent-browser.mjs";

const root=fileURLToPath(new URL("../../../",import.meta.url));
const output=new URL("../../../target/examples/wasm/",import.meta.url);
const fixture=new URL("watch-song.ts",output);
await mkdir(output,{recursive:true});
const source=(await readFile(new URL("../src/song.ts",import.meta.url),"utf8"))
  .replace('"@oxitone/web"',JSON.stringify(fileURLToPath(new URL("../../../packages/web/src/index.ts",import.meta.url))));
await writeFile(fixture,source);
const server=spawn(process.execPath,[fileURLToPath(new URL("./dev.mjs",import.meta.url))],{
  cwd:root,env:{...process.env,OXITONE_WEB_PORT:"0",OXITONE_WEB_SONG_ENTRY:fileURLToPath(fixture)},
  stdio:["ignore","pipe","pipe"],
});
let browser;
try{
  const url=await new Promise((resolve,reject)=>{
    let log="";const timeout=setTimeout(()=>reject(new Error("watch server timed out: "+log)),30000);
    server.stdout.on("data",chunk=>{log+=chunk;const match=log.match(/http:\/\/127\.0\.0\.1:\d+/);if(match){clearTimeout(timeout);resolve(match[0]);}});
    server.stderr.on("data",chunk=>{log+=chunk;});
    server.on("exit",code=>{clearTimeout(timeout);reject(new Error("watch server exited "+code+": "+log));});
  });
  browser=await launchSilentBrowser();
  const page=await silentPage(browser);await page.goto(url);await page.click("#play");
  await page.waitForFunction(()=>window.oxitone && document.querySelector("#play").textContent==="Pause");
  const gain=()=>page.evaluate(()=>window.oxitone.project.master.level);
  const active=async()=>{
    await page.waitForFunction(()=>{const data=new Float32Array(1024);
      window.oxitone.analyser.getFloatTimeDomainData(data);return data.some(v=>Math.abs(v)>0.0001);},{},{timeout:5000});
    return page.evaluate(async()=>(await window.oxitone.session.state()).state==="playing");
  };
  await writeFile(fixture,source.replace("p.master.level=0.8","p.master.level=0.42"));
  await page.waitForFunction(()=>document.querySelector("#status").textContent==="Code updated · Rust graph accepted");
  assert.equal(await gain(),0.42);assert(await active());
  await writeFile(fixture,source+"\nconst unfinished =;\n");
  await page.waitForFunction(()=>document.querySelector("#status").textContent.startsWith("Build error"));
  assert.equal(await gain(),0.42);assert(await active());
  await writeFile(fixture,source.replace("return p;","throw new Error('watch runtime failure');"));
  await page.waitForFunction(()=>document.querySelector("#status").textContent.includes("watch runtime failure"));
  assert.equal(await gain(),0.42);assert(await active());
  await writeFile(fixture,source);
  await page.waitForFunction(()=>document.querySelector("#status").textContent==="Code updated · Rust graph accepted");
  assert.equal(await gain(),0.8);assert(await active());
  const report=await page.evaluate(()=>({validUpdate:true,syntaxFailureRetainsGraph:true,
    runtimeFailureRetainsGraph:true,recovery:true,diagnostics:window.oxitone.session.diagnostics(),
    audioSinks:window.oxitoneTestAudioSinks}));
  assert(report.audioSinks.length>0 && report.audioSinks.every(context=>context.sink.type==="none"));
  await writeFile(new URL("watch-report.json",output),JSON.stringify(report,null,2)+"\n");
  console.log(JSON.stringify(report));
}finally{
  await browser?.close();server.kill("SIGTERM");
  await writeFile(fixture,source);
}
