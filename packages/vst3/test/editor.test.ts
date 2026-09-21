import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { editVst3Plugin } from "../src/editor.js";

it.skipIf(process.platform !== "darwin")(
  "validates editor Apply/Cancel, identity and request bounds without opening a window",
  async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-editor-contract-"));
    try {
      const hostPath = join(root, "host.mjs");
      await writeFile(
        hostPath,
        `#!${process.execPath}
import {readFileSync,writeSync} from 'node:fs';
const r=JSON.parse(readFileSync(0,'utf8'));
if(r.operation!=='edit'||r.options.sampleRate!==48000)throw new Error('bad editor request');
let reply={protocolVersion:1,accepted:false};
if(r.options.parameters['9']!==undefined){
 const classId=r.source.classId,sha256='a'.repeat(64);
 reply={protocolVersion:1,accepted:true,info:{protocolVersion:1,classId,sha256,name:'Fixture',vendor:'Test',version:'1',category:'Fx',inputChannels:2,outputChannels:2, audioBuses: { inputs: [{ channels: 2, active: true }], outputs: [{ channels: 2, active: true }] },noteInput:false,noteOutput:false,parameters:[],configuration:{formatVersion:1,classId,sha256,stateBase64:'AQID',parameters:r.options.parameters}}};
}
writeSync(3,JSON.stringify(reply));
`,
      );
      await chmod(hostPath, 0o700);
      const source = { bundlePath: "/tmp/Test.vst3", classId: "1".repeat(32) };
      expect(await editVst3Plugin(source, {}, { hostPath })).toBeNull();
      expect(
        (await editVst3Plugin(source, { parameters: { "9": 0.2 } }, { hostPath }))!.configuration!.parameters,
      ).toEqual({ "9": 0.2 });
      await expect(
        editVst3Plugin({ ...source, expectedHash: "b".repeat(64) }, { parameters: { "9": 0.2 } }, { hostPath }),
      ).rejects.toMatchObject({ code: "PluginManifestMismatch" });
      await expect(editVst3Plugin(source, { parameters: { "9": 2 } }, { hostPath })).rejects.toMatchObject({
        code: "PluginConfigInvalid",
      });
      const controller = new AbortController();
      controller.abort();
      await expect(editVst3Plugin(source, {}, { hostPath, signal: controller.signal })).rejects.toMatchObject({
        name: "AbortError",
      });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);
