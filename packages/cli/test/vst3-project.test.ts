import { afterEach, expect, it, vi } from "vitest";
import { chmod, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { ProjectDocument } from "../src/source/index.js";
import { vst3RenderOptionsSchema } from "@oxitone/protocol";
import { loadVst3Preset } from "@oxitone/vst3";

const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => {
  vi.unstubAllEnvs();
  for (const cleanup of cleanups.splice(0).reverse()) await cleanup();
});
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-project-"));
  cleanups.push(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts");
  await writeFile(entry, "import { Project } from '@oxitone/core'; export default new Project({name:'Song'});\n");
  const host = join(root, "fake-host.mjs");
  await writeFile(
    host,
    `#!${process.execPath}
import {readFileSync,writeFileSync,writeSync} from 'node:fs';
import {createHash} from 'node:crypto';
const request=JSON.parse(readFileSync(0,'utf8')), sha256='a'.repeat(64), classId=request.source.classId;
const info={protocolVersion:1,classId,name:'Fixture',vendor:'Test',version:'1',category:'Fx',sha256,
inputChannels:2,outputChannels:2, audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [{ channels: 2, active: true }] },noteInput:false,noteOutput:false,parameters:[{id:9,name:'Gain',unit:'',value:0.5,default:0.5,stepCount:0,canAutomate:true,readOnly:false}],
configuration:{formatVersion:1,classId,sha256,stateBase64:'AQID',parameters:{'9':0.5}}};
let reply=info;
if(request.operation==='render'){
 const o=request.options, frames=o.frames+o.tailFrames,b=Buffer.alloc(44+frames*8);
 b.write('RIFF');b.writeUInt32LE(b.length-8,4);b.write('WAVEfmt ',8);b.writeUInt32LE(16,16);
 b.writeUInt16LE(3,20);b.writeUInt16LE(2,22);b.writeUInt32LE(o.sampleRate,24);b.writeUInt32LE(o.sampleRate*8,28);
 b.writeUInt16LE(8,32);b.writeUInt16LE(32,34);b.write('data',36);b.writeUInt32LE(frames*8,40);
 for(let i=0;i<frames*2;i++) b.writeFloatLE(0.25,44+i*4);
 writeFileSync(o.path,b);
 reply={protocolVersion:1,path:o.path,frames,sampleRate:o.sampleRate,sha256:createHash('sha256').update(b).digest('hex'),pluginSha256:sha256,latencyFrames:0,tailFrames:0,peak:0.25};
}else if(request.operation==='configure'){
 const configuration=request.options.configuration;
 if(!configuration)throw new Error('expected saved configuration');
 info.configuration={...configuration,parameters:{...configuration.parameters,...request.options.parameters}};
 info.parameters=info.parameters.map(p=>({...p,value:info.configuration.parameters[p.id]??p.value}));
}else if(request.operation!=='inspect'){
 throw new Error('unsupported helper operation');
}
writeSync(3,JSON.stringify(reply));
`,
  );
  await chmod(host, 0o700);
  vi.stubEnv("OXITONE_VST3_HOST_PATH", host);
  const doc = await ProjectDocument.open({ entry });
  cleanups.push(async () => doc.close());
  expect(doc.view.status, doc.view.diagnostic?.message).toBe("ready");
  await doc.vst3Command(0, { kind: "add", source: { bundlePath: "/tmp/Fixture.vst3", classId: "1".repeat(32) } });
  const plugin = doc.view.plugins.find((p) => p.source === "vst3")!.handle;
  await doc.verifyPlugin(0, plugin);
  const output = join(root, "original.wav");
  await doc.vst3Command(0, {
    kind: "render",
    plugin,
    options: vst3RenderOptionsSchema.parse({ path: output, frames: 4800, tailFrames: 17 }),
  });
  return { root, entry, doc, plugin, output };
}

it.skipIf(process.platform !== "darwin")(
  "persists a pinned preset and restores its parameter values after fresh load/refresh",
  async () => {
    const { root, doc, plugin } = await fixture();
    const path = join(root, "gain.oxivst3.json");
    await doc.vst3Command(0, { kind: "savePreset", plugin, path, parameters: { "9": 0.2 } });
    expect((await loadVst3Preset(path)).configuration.parameters["9"]).toBe(0.2);
    await doc.vst3Command(0, { kind: "remove", plugin });
    await doc.vst3Command(0, { kind: "loadPreset", path });
    const loaded = doc.view.plugins.find((p) => p.vst3?.presetPath === path)!;
    expect(loaded.validation).toBe("unverified");
    await doc.verifyPlugin(0, loaded.handle);
    expect(doc.view.plugins.find((p) => p.handle === loaded.handle)!.vst3!.parameters[0]!.value).toBe(0.2);
    await doc.refreshPlugins(0);
    await doc.verifyPlugin(0, loaded.handle);
    expect(doc.view.plugins.find((p) => p.handle === loaded.handle)!.vst3!.parameters[0]!.value).toBe(0.2);
    expect(doc.view.revision).toBe(0);
  },
);
it.skipIf(process.platform !== "darwin")(
  "adds frozen audio through Undo/Redo/Save and reopens after original WAV and helper disappear",
  async () => {
    const { root, entry, doc, plugin, output } = await fixture();
    const original = await readFile(entry, "utf8");
    await doc.vst3Command(0, { kind: "attachRender", plugin, name: "VST3 print", startBeat: 4 });
    expect(doc.view.revision).toBe(1);
    expect(doc.frame!.snapshot.samples[0]!.frames).toBe("4817");
    expect(doc.frame!.snapshot.tracks[0]!.name).toBe("VST3 print");
    const asset = join(root, doc.frame!.snapshot.samples[0]!.assetUri);
    expect(doc.frame!.snapshot.samples[0]!.assetUri).toMatch(/^assets\/vst3\/[a-f0-9]{64}\.wav$/);
    expect(await readFile(entry, "utf8")).toBe(original);
    await doc.undo(1);
    expect(doc.frame!.snapshot.samples).toEqual([]);
    expect((await readFile(asset)).length).toBeGreaterThan(44);
    await doc.redo(2);
    await doc.save(3);
    expect(await readFile(entry, "utf8")).toContain(".importAudio(");
    await rm(output);
    vi.stubEnv("OXITONE_VST3_HOST_PATH", "/missing-helper");
    const reopened = await ProjectDocument.open({ entry });
    try {
      expect(reopened.view.status, reopened.view.diagnostic?.message).toBe("ready");
      expect(reopened.frame!.snapshot.sampleClips[0]!.startBeat).toEqual({ numerator: 4, denominator: 1 });
      expect(reopened.frame!.snapshot.samples[0]!.sha256).toBe(doc.frame!.snapshot.samples[0]!.sha256);
    } finally {
      reopened.close();
    }
  },
);
it.skipIf(process.platform !== "darwin")("rejects changed WAVs without changing source or revision", async () => {
  const { doc, plugin, output } = await fixture();
  const before = doc.view.files;
  const bytes = await readFile(output);
  bytes.writeFloatLE(0.8, 44);
  await writeFile(output, bytes);
  await expect(doc.vst3Command(0, { kind: "attachRender", plugin, name: "Print", startBeat: 0 })).rejects.toMatchObject(
    { code: "SourceChanged" },
  );
  expect(doc.view.revision).toBe(0);
  expect(doc.view.files).toEqual(before);
});

it.skipIf(process.platform !== "darwin")(
  "rejects asset directory symlinks before creating files outside the project",
  async () => {
    const { root, doc, plugin } = await fixture();
    const external = await mkdtemp(join(tmpdir(), "oxitone-vst3-external-"));
    cleanups.push(() => rm(external, { recursive: true, force: true }));
    await symlink(external, join(root, "assets"), "dir");
    await expect(
      doc.vst3Command(0, { kind: "attachRender", plugin, name: "Print", startBeat: 0 }),
    ).rejects.toMatchObject({ code: "AssetUnavailable" });
    expect(doc.view.revision).toBe(0);
    expect(doc.frame!.snapshot.samples).toEqual([]);
    await expect(readFile(join(external, "vst3"))).rejects.toMatchObject({ code: "ENOENT" });
  },
);
