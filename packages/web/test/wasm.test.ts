import { readFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { Pattern, Project, wavetable, sampler, createAutomationNamespace } from "@oxitone/core";
import { WasmEngine } from "../src/wasm.js";

const engines: WasmEngine[] = [];
async function engine() {
  const e = await WasmEngine.create(await readFile(new URL("../dist/oxitone.wasm", import.meta.url)));
  engines.push(e); return e;
}
function song() {
  const p = new Project({ seed: 59 });
  const c = p.addChannel({ instrument: wavetable({ oscA: { wave: "saw" } }), level: 0.4,
    effectChain: [{pluginId:"oxitone.filter",pluginVersion:"1.0.0",parameters:{cutoffHz:1800,resonance:0.707},mix:0.65}] });
  p.addTrack().use(c).add(new Pattern({ lengthBeats:4, notes: [
    {pitch:60,start:0,duration:0.6,velocity:0.8}, {pitch:67,start:1,duration:0.5,velocity:0.7},
    {pitch:63,start:2,duration:1,velocity:0.7} ] })).at({bar:1});
  return p;
}
afterEach(() => { for (const e of engines.splice(0)) e.dispose(); });

describe("actual import-free Wasm engine", () => {
  it("renders finite PCM without allocation/free/memory growth, and handles transport", async () => {
    const e = await engine(); e.compile(song()); e.transport({command:"play",loop:{startFrame:0,endFrame:96000}});
    const before = e.memoryDiagnostics(); let peak = 0, nonFinite = 0;
    for(let i=0;i<1000;i++) for(const channel of e.process()) for(const value of channel) {
      if (!Number.isFinite(value)) nonFinite++;
      peak = Math.max(peak, Math.abs(value));
    }
    expect(nonFinite).toBe(0); expect(peak).toBeGreaterThan(0.01); expect(e.memoryDiagnostics()).toEqual(before);
    e.transport({command:"pause"}); expect(e.process()[0].every(v=>v===0)).toBe(true);
    e.transport({command:"seek",frame:24000}); expect(e.state().cursor).toBe("24000");
    e.transport({command:"stop"}); expect(e.state().cursor).toBe("0");
  });
  it("retains the last good graph after Rust validation and version failures", async () => {
    const e = await engine(), p = song(); e.compile(p); e.transport({command:"play"}); e.process();
    const before=e.state(), broken=p.snapshot(); broken.channels[0]!.mixerChannelId="mix_missing";
    expect(()=>e.compile(broken)).toThrow(); expect(e.state()).toEqual(before);
    expect(()=>e.command({type:"compile",snapshot:{...p.snapshot(),protocolVersion:"99.0"}})).toThrow();
    e.process(); expect(BigInt(e.state().cursor)).toBe(BigInt(before.cursor)+128n);
    expect(()=>e.transport({command:"seek",frame:-1})).toThrow();
    expect(()=>e.command({type:"transport",command:"seek",frame:"9007199254740992"})).toThrow();
    expect(e.resolveBeatDuration(p,0,1)).toBe(2);
    expect(()=>e.setParameter("chn_missing","level",0.5)).toThrow();
    e.setParameter(p.channels[0]!.id,"level",0,BigInt(e.state().cursor));
    expect(()=>e.process()).not.toThrow();
  });
  it("matches the native WAV encoder/DSP and keeps offline exports independent of transport", async () => {
    const e=await engine(), p=song(), dir=await mkdtemp(join(tmpdir(),"oxitone-wasm-"));
    try {
      const channel = p.channels[0]!, motion = createAutomationNamespace();
      channel.instrumentInstance.param("level").automate(motion.constant(.2));
      const original = channel.effectInstances[0]!;
      const added = channel.addEffect(channel.effectChain[0]!);
      original.host.param("mix").automate(motion.constant(.3));
      channel.reorderEffects([added, original]);
      e.compile(p); e.transport({command:"play",frame:1234}); const before=e.state();
      const wav=e.renderWav({frames:96000,bitDepth:32});
      await p.renderWav({path:join(dir,"native.wav"),end:{frames:"96000"},tailSeconds:0,bitDepth:"float32",dither:"none"});
      const native=await readFile(join(dir,"native.wav"));
      expect(wav.length).toBe(native.length); expect([...wav.slice(0,44)]).toEqual([...native.subarray(0,44)]);
      const a=new DataView(wav.buffer,wav.byteOffset), b=new DataView(native.buffer,native.byteOffset);
      let max=0;
      for(let i=44;i<wav.length;i+=4) max=Math.max(max,Math.abs(a.getFloat32(i,true)-b.getFloat32(i,true)));
      expect(max).toBeLessThan(0.0002); expect(e.state()).toEqual(before);
      expect(new TextDecoder().decode(e.exportMidi().slice(0,4))).toBe("MThd");
      for(const bitDepth of [16,24] as const) {
        const first=e.renderWav({frames:257,bitDepth});
        expect(e.renderWav({frames:257,bitDepth})).toEqual(first);
        expect(first.length).toBe(44+257*2*bitDepth/8);
      }
    } finally { await rm(dir,{recursive:true,force:true}); }
  });
  it("decodes and hash-validates in-memory sampler resources", async () => {
    const e=await engine(); e.compile(song());
    const bytes=e.renderWav({frames:24000}); const info=e.importSample(bytes,"wav");
    const p=new Project(), sample=p.addSample({...info,frames:BigInt(info.frames),assetUri:"memory:keys"});
    const c=p.addChannel({instrument:sampler(sample)});
    p.addTrack().use(c).add(new Pattern({lengthBeats:4,notes:[{pitch:60,start:0,duration:1,velocity:1}]})).at({bar:1});
    e.compile(p); e.transport({command:"play"}); let peak=0;
    for(let i=0;i<100;i++) for(const v of e.process()[0]) peak=Math.max(peak,Math.abs(v));
    expect(peak).toBeGreaterThan(0.01);
    const broken=p.snapshot(); broken.samples[0]!.sha256="0".repeat(64);
    expect(()=>e.compile(broken)).toThrow(/upload sample bytes/);
    broken.samples[0]!.sha256=info.sha256; broken.samples[0]!.frames="1";
    expect(()=>e.compile(broken)).toThrow(/metadata mismatch/);
  });
  it("executes the same statically linked C ABI drum instrument", async () => {
    const e=await engine(), p=new Project();
    const c=p.addChannel({instrument:{pluginId:"example.drums",pluginVersion:"1.0.0",parameters:{}}});
    p.addTrack().use(c).add(new Pattern({lengthBeats:4,notes:[{pitch:36,start:0,duration:0.1,velocity:1}]})).at({bar:1});
    e.compile(p); e.transport({command:"play"}); const before=e.memoryDiagnostics(); let peak=0;
    for(let i=0;i<300;i++) for(const v of e.process()[0]) peak=Math.max(peak,Math.abs(v));
    expect(peak).toBeGreaterThan(0.01); expect(e.memoryDiagnostics()).toEqual(before);
  });
});
