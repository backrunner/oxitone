import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { configureVst3Plugin } from "../src/configure.js";

it.skipIf(process.platform !== "darwin")(
  "validates configured helper results, identity, abort and request bounds",
  async () => {
    const root = await mkdtemp(join(tmpdir(), "oxitone-configure-contract-"));
    try {
      const hostPath = join(root, "host.mjs");
      await writeFile(
        hostPath,
        `#!${process.execPath}
import {readFileSync,writeSync} from 'node:fs';
const r=JSON.parse(readFileSync(0,'utf8'));
if(r.operation!=='configure'||r.options.sampleRate!==48000||r.options.blockSize!==128)throw new Error('bad request');
const classId=r.source.classId,sha256='a'.repeat(64);
writeSync(3,JSON.stringify({protocolVersion:1,classId,sha256,name:'Dynamic',vendor:'Test',version:'1',category:'Fx',inputChannels:2,outputChannels:2,audioBuses:{inputs:[{channels:2,active:true}],outputs:[{channels:2,active:true},{channels:1,active:false}]},noteInput:false,noteOutput:false,parameters:[],configuration:{formatVersion:1,classId,sha256,stateBase64:'AQID',parameters:r.options.parameters}}));
`,
      );
      await chmod(hostPath, 0o700);
      const source = { bundlePath: "/tmp/Test.vst3", classId: "1".repeat(32) };
      const info = await configureVst3Plugin(source, { parameters: { "7": 0.2 } }, { hostPath });
      expect(info.audioBuses.outputs).toHaveLength(2);
      expect(info.configuration!.parameters).toEqual({ "7": 0.2 });
      await expect(
        configureVst3Plugin(
          source,
          {
            configuration: { ...info.configuration!, classId: "2".repeat(32) },
          },
          { hostPath: "/must-not-execute" },
        ),
      ).rejects.toMatchObject({ code: "PluginManifestMismatch" });
      await expect(
        configureVst3Plugin({ ...source, expectedHash: "b".repeat(64) }, {}, { hostPath }),
      ).rejects.toMatchObject({ code: "PluginManifestMismatch" });
      await expect(
        configureVst3Plugin(
          source,
          { configuration: { ...info.configuration!, sha256: "b".repeat(64) } },
          { hostPath },
        ),
      ).rejects.toMatchObject({ code: "PluginManifestMismatch" });
      for (const parameters of [{ "07": 0.2 }, { "7": 2 }])
        await expect(configureVst3Plugin(source, { parameters }, { hostPath })).rejects.toMatchObject({
          code: "PluginConfigInvalid",
        });
      const abort = new AbortController();
      abort.abort();
      await expect(configureVst3Plugin(source, {}, { hostPath, signal: abort.signal })).rejects.toMatchObject({
        name: "AbortError",
      });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);
