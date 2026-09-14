import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { ProjectDocument } from "../src/source/index.js";
import { DocumentDispatcher } from "../src/source/document/document-dispatch.js";

it("saves coalesced mixer/tempo/track edits, copy/resize/delete and plugin assignments across Undo and reopen", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-controls-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const entry = join(root, "song.ts");
    await writeFile(
      entry,
      `import { Project, chord } from '@oxitone/core';
export const label = 'Session';
export default async function create() {
  const project = new Project();
  const a = project.addChannel({ id: 'chn_z', name: 'Lead' });
  project.addChannel({ id: 'chn_a', name: 'Bass' });
  project.addTrack('Main').use(a).pattern(chord(60, 'major')).at({ bar: 1 }).transpose(12).velocityScale(.7);
  return project;
}
`,
    );
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    const lead = () => document!.frame!.snapshot.channels.find((c) => c.name === "Lead")!;
    const config = async (values: { level?: number; pan?: number }) =>
      document!.configure(document!.view.revision, { kind: "channel", index: 0, values });
    await config({ level: 0.5 });
    await config({ level: 0.7 });
    await config({ pan: -0.3 });
    expect(lead()).toMatchObject({ level: 0.7, pan: -0.3 });
    expect(document.frame!.snapshot.channels.find((c) => c.name === "Bass")!.level).toBe(1);
    const draft = document.view.files[0]!.text,
      revision = document.view.revision;
    expect(draft.match(/\.configure\(/g)).toHaveLength(1);
    await config({ pan: -0.3 });
    expect(document.view.revision).toBe(revision);
    expect(document.view.files[0]!.text).toBe(draft);
    await document.configure(document.view.revision, { kind: "tempo", bpm: 132 });
    await document.configure(document.view.revision, { kind: "track", index: 0, enabled: false });
    await document.configure(document.view.revision, { kind: "track", index: 0, enabled: true });
    await document.configure(document.view.revision, { kind: "track", index: 0, mute: true });
    await document.configure(document.view.revision, { kind: "track", index: 0, solo: true });
    expect(document.frame!.snapshot.tracks[0]).toMatchObject({ mute: true, solo: true });
    await document.undo(document.view.revision);
    expect(document.frame!.snapshot.tracks[0]).toMatchObject({ mute: true });
    expect(document.frame!.snapshot.tracks[0]!.solo).not.toBe(true);
    await document.redo(document.view.revision);
    expect(lead().mute).not.toBe(true);
    expect(lead().solo).not.toBe(true);
    await document.arrange(document.view.revision, {
      action: "duplicate",
      kind: "pattern",
      resource: 0,
      clip: 0,
      track: 0,
      startBeat: 4,
    });
    expect(document.frame!.snapshot.patternClips).toHaveLength(2);
    expect(document.frame!.snapshot.patternClips[1]).toMatchObject({ transpose: 12, velocityScale: 0.7 });
    await document.arrange(document.view.revision, {
      action: "resize",
      kind: "pattern",
      resource: 0,
      clip: 1,
      durationBeats: 2,
    });
    await document.arrange(document.view.revision, {
      action: "enable",
      kind: "pattern",
      resource: 0,
      clip: 1,
      enabled: false,
    });
    await document.arrange(document.view.revision, {
      action: "enable",
      kind: "pattern",
      resource: 0,
      clip: 1,
      enabled: true,
    });
    const plugin = document.view.plugins.find((p) => p.pluginId === "oxitone.delay")!;
    await document.assignPlugin(document.view.revision, {
      plugin: plugin.handle,
      target: "channelInsert",
      owner: lead().id,
    });
    const first = lead().effectChain[0]!.instanceId;
    const setting = document.view.configurationSites.find((s) => s.usages.some((u) => u.handle === first))!;
    expect(setting, "newly added plugin keeps an editable source configuration").toBeDefined();
    await document.editConfiguration(
      document.view.revision,
      setting.handle,
      { kind: "parameters", values: { feedback: 0.45 } },
      first,
    );
    expect(lead().effectChain[0]!.parameters.feedback).toBe(0.45);
    await document.assignPlugin(document.view.revision, {
      plugin: plugin.handle,
      target: "channelInsert",
      owner: lead().id,
    });
    const ownerSite = document.view.effectOwnerSites!.find((s) => s.owner === lead().id && s.scope === "definition")!;
    await document.editEffectOrder(
      document.view.revision,
      ownerSite.handle,
      lead().id,
      lead()
        .effectChain.map((e) => e.instanceId!)
        .reverse(),
    );
    expect(lead().effectChain[1]!.instanceId).toBe(first);
    await document.configure(document.view.revision, {
      kind: "effectOrder",
      owner: "channel",
      index: 0,
      order: [1, 0],
    });
    expect(lead().effectChain[0]!.instanceId).toBe(first);
    await document.configure(document.view.revision, { kind: "effect", owner: "channel", index: 0, slot: 1 });
    await document.assignPlugin(document.view.revision, {
      plugin: plugin.handle,
      target: "channelInsert",
      owner: lead().id,
      instance: first,
    });
    expect(lead().effectChain[0]!.instanceId).not.toBe(first);
    expect(
      document.view.configurationSites.some((s) =>
        s.usages.some((u) => u.handle === lead().effectChain[0]!.instanceId),
      ),
    ).toBe(true);
    await document.undo(document.view.revision);
    expect(lead().effectChain[0]!.instanceId).toBe(first);
    await document.redo(document.view.revision);
    await document.configure(document.view.revision, { kind: "effect", owner: "channel", index: 0, slot: 0 });
    expect(lead().effectChain).toHaveLength(0);
    await document.arrange(document.view.revision, { action: "remove", kind: "pattern", resource: 0, clip: 1 });
    const snapshot = document.frame!.snapshot;
    await document.save(document.view.revision);
    const saved = await readFile(entry, "utf8");
    expect(saved).toContain("export const label = 'Session'");
    expect(saved).not.toMatch(/instanceId|ins_[a-z0-9]|__oxitone/);
    document.close();
    document = await ProjectDocument.open({ entry });
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
    expect({ ...document.frame!.snapshot, revision: 0 }).toEqual({ ...snapshot, revision: 0 });
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
});

it("repairs an invalid missing-package draft through the real dispatcher and keeps failure invalid", async () => {
  const root = await mkdtemp(join(tmpdir(), "oxitone-controls-repair-"));
  let document: ProjectDocument | undefined;
  try {
    await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
    await symlink(
      fileURLToPath(new URL("../../core", import.meta.url)),
      join(root, "node_modules/@oxitone/core"),
      "dir",
    );
    const entry = join(root, "song.ts");
    await writeFile(join(root, "package.json"), '{"name":"repair-test","dependencies":{}}');
    await writeFile(
      entry,
      "import { Project } from '@oxitone/core'; import { bpm } from 'missing-pattern'; export default new Project({ bpm });",
    );
    let fail = true;
    document = await ProjectDocument.open({
      entry,
      pluginTaskRunner: {
        async run() {
          if (fail) throw new Error("registry down");
          const pkg = join(root, "node_modules/missing-pattern");
          await mkdir(pkg);
          await writeFile(
            join(pkg, "package.json"),
            '{"name":"missing-pattern","type":"module","exports":"./index.js"}',
          );
          await writeFile(join(pkg, "index.js"), "export const bpm = 135;");
        },
      },
    });
    expect(document.view.status).toBe("invalid");
    const dispatcher = new DocumentDispatcher(document);
    const request = (sequence: number) => ({
      documentProtocolVersion: "2.0",
      sessionId: document!.sessionId,
      requestId: `stream/test/${sequence}`,
      baseRevision: document!.view.revision,
      operation: { kind: "installPlugin", packageName: "missing-pattern" },
    });
    expect((await dispatcher.submit(request(1))).accepted).toBe(false);
    expect(document.view.status).toBe("invalid");
    fail = false;
    expect((await dispatcher.submit(request(2))).accepted).toBe(true);
    expect(document.view.status, document.view.diagnostic?.message).toBe("ready");
  } finally {
    document?.close();
    await rm(root, { recursive: true, force: true });
  }
});
