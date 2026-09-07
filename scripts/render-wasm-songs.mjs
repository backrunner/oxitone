import assert from "node:assert/strict";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { platform, arch, cpus } from "node:os";
import { WasmEngine } from "../packages/web/dist/wasm.js";

const root=resolve("target/examples/full-songs"), out=resolve("target/examples/wasm");
await mkdir(out,{recursive:true});
const module=await WebAssembly.compile(await readFile("packages/web/dist/oxitone.wasm"));
const nativeReport=JSON.parse(await readFile(resolve(root,"report.json"),"utf8"));
const report={runtime:process.version,platform:platform(),arch:arch(),cpu:cpus()[0]?.model,
  imports:WebAssembly.Module.imports(module),songs:[]};
for(const song of nativeReport.songs){
  const snapshot=JSON.parse(await readFile(resolve(root,song.slug+".snapshot.json"),"utf8"));
  const engine=await WasmEngine.create(module);
  try{
    const seen=new Set();
    for(const sample of snapshot.samples){
      if(seen.has(sample.sha256))continue;
      const info=engine.importSample(await readFile(sample.assetUri),sample.format);
      assert.equal(info.sha256,sample.sha256);seen.add(sample.sha256);
    }
    const state=engine.compile(snapshot), frames=Math.round(song.durationSeconds*state.sampleRate);
    // Exercise the busiest song region and include the C ABI drum path.
    engine.transport({command:"play",frame:Math.round(48*60/song.bpm*state.sampleRate)});
    for(let i=0;i<200;i++)engine.process();
    const memoryBefore=engine.memoryDiagnostics(), times=[];
    let peak=0;
    for(let i=0;i<4000;i++){
      const start=performance.now(),pcm=engine.process();times.push(performance.now()-start);
      for(const channel of pcm)for(const value of channel){assert(Number.isFinite(value));peak=Math.max(peak,Math.abs(value));}
    }
    const memoryAfter=engine.memoryDiagnostics();
    assert(peak>0.01);assert.deepEqual(memoryAfter,memoryBefore);
    const start=performance.now(),wav=engine.renderWav({frames,bitDepth:24,dither:true});
    const renderSeconds=(performance.now()-start)/1000;
    await writeFile(resolve(out,song.slug+".wav"),wav);
    await writeFile(resolve(out,song.slug+".mid"),engine.exportMidi());
    const native=await readFile(resolve(root,song.slug+".wav"));assert.equal(wav.length,native.length);
    function pcm24(bytes,index){const v=bytes[index]|bytes[index+1]<<8|bytes[index+2]<<16;return (v&0x800000?v-0x1000000:v)/8388608;}
    let maxPcmDifference=0,square=0,fullPeak=0;
    const sectionLevels=song.sections.map(([name,bar],i)=>{
      const begin=Math.round(bar*4*60/song.bpm*state.sampleRate),end=Math.min(frames,Math.round((song.sections[i+1]?.[1]??song.bars)*4*60/song.bpm*state.sampleRate));
      let sum=0;
      for(let frame=begin;frame<end;frame++)for(let ch=0;ch<2;ch++){const value=pcm24(wav,44+(frame*2+ch)*3);sum+=value*value;}
      const rmsDbfs=10*Math.log10(Math.max(1e-20,sum/((end-begin)*2)));
      assert(rmsDbfs>-52);return {name,rmsDbfs};
    });
    for(let i=44;i<wav.length;i+=3){
      const value=pcm24(wav,i);square+=value*value;fullPeak=Math.max(fullPeak,Math.abs(value));
      maxPcmDifference=Math.max(maxPcmDifference,Math.abs(value-pcm24(native,i)));
    }
    times.sort((a,b)=>a-b);
    assert(maxPcmDifference<0.0002,"Wasm/native PCM difference exceeds tolerance");
    assert(fullPeak>0.01 && fullPeak<1,"Unexpected full-song peak");
    const item={title:song.title,slug:song.slug,seconds:frames/state.sampleRate,sampleRate:state.sampleRate,
      blockSize:state.blockSize,samples:seen.size,renderSeconds,realtimeMultiple:frames/state.sampleRate/renderSeconds,
      p95Ms:times[Math.floor(times.length*.95)],p99Ms:times[Math.floor(times.length*.99)],
      processBlocks:times.length,memoryBefore,memoryAfter,processAllocations:0,processDeallocations:0,
      peakDbfs:20*Math.log10(fullPeak),rmsDbfs:10*Math.log10(square/((wav.length-44)/3)),maxPcmDifference,sectionLevels};
    report.songs.push(item);console.log(JSON.stringify(item));
  }finally{engine.dispose();}
}
await writeFile(resolve(out,"report.json"),JSON.stringify(report,null,2)+"\n");
