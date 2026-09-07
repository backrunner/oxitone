import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { launchSilentBrowser, silentPage } from "./silent-browser.mjs";
const output=new URL("../../../target/examples/wasm/",import.meta.url);
await mkdir(output,{recursive:true});
const browser=await launchSilentBrowser();
const page=await silentPage(browser,{viewport:{width:1280,height:900},colorScheme:"dark"});
const errors=[];page.on("pageerror",error=>errors.push(error.message));
const report={browser:browser.version(),songs:[]};
async function measure(){
  return page.evaluate(async()=>{
    let peak=0,square=0,samples=0;
    for(let i=0;i<40;i++){
      const data=new Float32Array(1024);
      window.oxitone.analyser.getFloatTimeDomainData(data);
      for(const v of data){peak=Math.max(peak,Math.abs(v));square+=v*v;samples++;}
      await new Promise(resolve=>setTimeout(resolve,25));
    }
    return {peak,rms:Math.sqrt(square/samples),diagnostics:window.oxitone.session.diagnostics(),
      state:await window.oxitone.session.state()};
  });
}
try{
  const url=process.env.OXITONE_WEB_URL??`http://127.0.0.1:${process.env.OXITONE_WEB_PORT??4173}`;
  console.log("[smoke] Web Audio at",url,"· no-device sink required; invalid-update rejection is intentional.");
  await page.goto(url);
  assert(await page.evaluate(()=>crossOriginIsolated));
  await page.click("#play");
  await page.waitForFunction(()=>window.oxitone && document.querySelector("#play").textContent==="Pause");
  await page.waitForTimeout(500);
  const initial=await measure();assert(initial.peak>0.01);report.songs.push({song:"glass",...initial});
  await page.click("#invalid");await page.waitForFunction(()=>document.querySelector("#status").textContent.includes("rejected"));
  assert((await measure()).peak>0.01);
  await page.click("#update");await page.waitForFunction(()=>document.querySelector("#status").textContent.includes("softer"));
  assert((await measure()).peak>0.01);
  await page.click("#play");await page.waitForTimeout(250);
  const paused=await measure();assert.equal(paused.peak,0);assert.equal(paused.state.state,"paused");
  await page.evaluate(()=>window.oxitone.session.seek(480000));
  const seek=await page.evaluate(()=>window.oxitone.session.state());assert.equal(seek.cursor,"480000");
  await page.click("#play");assert((await measure()).peak>0.01);
  await page.screenshot({path:new URL("web-audio-dark.png",output).pathname});
  await page.emulateMedia({colorScheme:"light"});
  await page.screenshot({path:new URL("web-audio-light.png",output).pathname});
  if(process.env.OXITONE_WEB_FULL_SONGS==="1"){
    for(const slug of ["after-the-horizon"]){
      console.log("Loading",slug,await page.locator("#song option").evaluateAll(options=>options.map(option=>option.value)));
      await page.selectOption("#song",{value:slug});
      await page.waitForFunction(()=>window.oxitone?.project?.name?.startsWith("After the Horizon") &&
        document.querySelector("#status").textContent==="Ready · Rust engine",slug,{timeout:60000});
      await page.click("#play");
      await page.evaluate(()=>window.oxitone.session.seek(48000*60));
      await page.waitForTimeout(500);
      const result=await measure();assert(result.peak>0.01);report.songs.push({song:slug,...result});
    }
  }
  report.errors=errors;assert.deepEqual(errors,[]);
  report.audioSinks=await page.evaluate(()=>window.oxitoneTestAudioSinks);
  assert(report.audioSinks.length>0 && report.audioSinks.every(context=>context.sink.type==="none"));
  report.disposed=await page.evaluate(async()=>{const s=window.oxitone.session;await s.dispose();await s.context.close();
    try{await s.state();return false;}catch{return s.context.state==="closed";}});
  assert(report.disposed);
  await writeFile(new URL("browser-report.json",output),JSON.stringify(report,null,2)+"\n");
  console.log(JSON.stringify(report,null,2));
}finally{await browser.close();}
