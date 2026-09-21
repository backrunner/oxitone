import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { evaluateSourceProject } from "../src/source/eval/project-evaluation.js";
import { SourceOwnership } from "../src/source/files/ownership.js";
import { writeVst3Recording } from "../src/source/editing/vst3-recording-writer.js";
import { assertArrangement } from "../src/source/editing/arrangement-writer.js";
import type { Vst3AutomationTake } from "@oxitone/core";
import ts from "typescript";

it.each(["instrument", "channelInsert", "busInsert"] as const)(
  "writes isolated editable sources for multiple %s parameters and gaps",
  async (kind) => {
    const root = await mkdtemp(join(tmpdir(), "oxi-record-source-"));
    try {
      await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
      await symlink(
        fileURLToPath(new URL("../../core", import.meta.url)),
        join(root, "node_modules/@oxitone/core"),
        "dir",
      );
      const entry = join(root, "song.ts");
      const config = JSON.stringify({ pluginId: `vst3.${"1".repeat(32)}`, pluginVersion: "1.0.0", parameters: {} });
      const setup =
        kind === "instrument"
          ? `project.addChannel({ instrument: ${config} });`
          : kind === "channelInsert"
            ? `project.addChannel({ effectChain: [${config}] });`
            : `project.addMixerChannel({ inserts: [${config}] });`;
      let text = `import { Project } from '@oxitone/core';
const AutomationSource = 'user binding must be preserved';
let calls = 0;
function song() {
  if (++calls !== 1) throw new Error('factory evaluated twice');
  const project = new Project({ seed: 31 });
  ${setup}
  return project;
}
export default song();
`;
      await writeFile(entry, text);
      const ownership = await SourceOwnership.open([root], [entry]);
      const evaluate = (text: string) =>
        evaluateSourceProject(entry, new Map([[entry, text]]), ownership, new AbortController().signal);
      let before = await evaluate(text);
      for (let pass = 0; pass < 2; pass++) {
        const site = before.configurationSites.find(
          (site) => site.usages.length === 1 && site.usages[0]!.kind === kind,
        )!;
        const take: Vst3AutomationTake = {
          target: { graphGeneration: "9", instanceId: site.usages[0]!.handle },
          recording: { mode: "write", parameterIds: [0, 9], sampleRate: 48000 },
          spans: [
            { parameterId: 0, start: 1, end: 2, value: 0.2 },
            { parameterId: 9, start: 1.5, end: 3, value: 0.7 },
            { parameterId: 0, start: 4, end: 5, value: 0.5 },
          ],
          source: () => {
            throw new Error("writer must preserve existing sources");
          },
        };
        const candidate = writeVst3Recording(before, new Map([[entry, text]]), site, take);
        text = candidate.files.get(entry)!;
        const after = await evaluate(text);
        assertArrangement(before.frame.snapshot, candidate.expected, after.frame.snapshot);
        expect(after.frame.snapshot.tracks.length - before.frame.snapshot.tracks.length).toBe(2);
        expect(
          after.frame.snapshot.automationClips!.length - (before.frame.snapshot.automationClips?.length ?? 0),
        ).toBe(3);
        for (const lane of after.frame.snapshot.automation) {
          expect(
            after.automationSites.some((site) => site.scope === "definition" && site.lanes.includes(lane.id)),
          ).toBe(true);
        }
        expect(text).not.toContain(take.target.instanceId);
        expect(text).toContain("const AutomationSource = 'user binding must be preserved'");
        const options: ts.CompilerOptions = {
          strict: true,
          noUncheckedIndexedAccess: true,
          noEmit: true,
          skipLibCheck: true,
          target: ts.ScriptTarget.ESNext,
          module: ts.ModuleKind.ESNext,
          moduleResolution: ts.ModuleResolutionKind.Bundler,
          types: [],
        };
        const host = ts.createCompilerHost(options);
        const getSourceFile = host.getSourceFile.bind(host);
        host.getSourceFile = (path, language, onError, create) =>
          path === entry
            ? ts.createSourceFile(path, text, language, true)
            : getSourceFile(path, language, onError, create);
        const program = ts.createProgram([entry], options, host);
        expect(
          ts
            .getPreEmitDiagnostics(program)
            .filter((d) => d.file?.fileName === entry)
            .map((d) => ts.flattenDiagnosticMessageText(d.messageText, "\n")),
        ).toEqual([]);
        before = after;
      }
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);
