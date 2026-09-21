import { afterEach, describe, expect, it } from "vitest";
import { chmod, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { inspectVst3Plugin, renderVst3Wav } from "../src/index.js";

const directories: string[] = [];
const source = { bundlePath: "/tmp/Fixture.vst3", classId: "1".repeat(32), allowPlugins: "any" as const };
afterEach(async () => {
  await Promise.all(directories.splice(0).map((path) => rm(path, { recursive: true, force: true })));
});
async function helper(code: string) {
  const root = await mkdtemp(join(tmpdir(), "oxitone-helper-test-"));
  directories.push(root);
  const hostPath = join(root, "helper.mjs");
  await writeFile(
    hostPath,
    `#!${process.execPath}\nimport {readFileSync,writeFileSync,writeSync} from 'node:fs';\nconst request=JSON.parse(readFileSync(0,'utf8'));\n${code}\n`,
  );
  await chmod(hostPath, 0o700);
  return { root, hostPath };
}

describe.skipIf(process.platform !== "darwin")("disposable helper process boundary", () => {
  it("distinguishes crashes, malformed messages and unsupported protocol", async () => {
    for (const [code, expected] of [
      ["process.kill(process.pid, 'SIGKILL');", "PluginHostCrashed"],
      ["writeSync(3, 'not json');", "RealtimeFault"],
      ["writeSync(3, JSON.stringify({protocolVersion:99}));", "ProtocolVersionUnsupported"],
      ["writeSync(3, JSON.stringify({protocolVersion:1,error:{code:'bogus',message:'bad'}}));", "RealtimeFault"],
    ]) {
      const host = await helper(code!);
      await expect(inspectVst3Plugin(source, host)).rejects.toMatchObject({ code: expected });
    }
  });
  it("bounds output and kills a stuck process on timeout or abort", async () => {
    const oversized = await helper("writeSync(3, Buffer.alloc(9*1024*1024, 32));");
    await expect(inspectVst3Plugin(source, oversized)).rejects.toMatchObject({ code: "BudgetExceeded" });
    const stuck = await helper("setInterval(()=>{},1000);");
    await expect(inspectVst3Plugin(source, { ...stuck, timeoutMs: 100 })).rejects.toMatchObject({
      code: "PluginHostTimeout",
    });
    const controller = new AbortController();
    const pending = inspectVst3Plugin(source, { ...stuck, signal: controller.signal });
    const assertion = expect(pending).rejects.toMatchObject({ name: "AbortError" });
    setTimeout(() => controller.abort(), 100);
    await assertion;
    await expect(inspectVst3Plugin(source, { ...stuck, signal: controller.signal })).rejects.toMatchObject({
      name: "AbortError",
    });
  });
  it("ignores stdout noise, publishes once and removes staging on all outcomes", async () => {
    const host = await helper(`
      process.stdout.write('plugin log, not a response');
      const options=request.options;
      writeFileSync(options.path, 'fixture bytes');
      writeSync(3, JSON.stringify({protocolVersion:1,path:options.path,frames:options.frames+options.tailFrames,
        sampleRate:options.sampleRate,sha256:'a'.repeat(64),pluginSha256:'b'.repeat(64),latencyFrames:0,tailFrames:0,peak:0}));
    `);
    const path = join(host.root, "out.wav");
    await expect(renderVst3Wav(source, { path, frames: 16 }, host)).resolves.toMatchObject({ path });
    await expect(renderVst3Wav(source, { path, frames: 16 }, host)).rejects.toMatchObject({ code: "AssetUnavailable" });
    expect(await readFile(path, "utf8")).toBe("fixture bytes");
    expect((await readdir(host.root)).sort()).toEqual(["helper.mjs", "out.wav"]);
  });
  it("does not publish a render with a mismatched response identity", async () => {
    const host = await helper(`
      writeFileSync(request.options.path, 'partial');
      writeSync(3,JSON.stringify({protocolVersion:1,path:'/wrong.wav',frames:16,sampleRate:48000,
        sha256:'a'.repeat(64),pluginSha256:'b'.repeat(64),latencyFrames:0,tailFrames:0,peak:0}));
    `);
    await expect(renderVst3Wav(source, { path: join(host.root, "out.wav"), frames: 16 }, host)).rejects.toMatchObject({
      code: "RealtimeFault",
    });
    expect(await readdir(host.root)).toEqual(["helper.mjs"]);
  });
  it("rejects valid-shaped inspection data for the wrong class", async () => {
    const host = await helper(`writeSync(3, JSON.stringify({protocolVersion:1,classId:'2'.repeat(32),name:'Fixture',
      vendor:'Fixture',version:'1',category:'Fx',sha256:'a'.repeat(64),inputChannels:2,outputChannels:2, audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [{ channels: 2, active: true }] },
      noteInput:false,noteOutput:false,parameters:[],configuration:null}));`);
    await expect(inspectVst3Plugin(source, host)).rejects.toMatchObject({ code: "RealtimeFault" });
  });
});
