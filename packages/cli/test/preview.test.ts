import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, writeFile, rename, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { expect, it } from "vitest";
import { previewResponseSchema, type PreviewFrame } from "@oxitone/protocol";
import { PreviewRunner } from "../src/preview/runner.js";
import { Client, until } from "./preview-helpers.js";

const cache = fileURLToPath(new URL("../node_modules/.cache/", import.meta.url));
const source = `import { Project, Pattern } from '@oxitone/core';
import { pitch, plugin } from './notes.js';
export default () => {
  const p = new Project({ name: 'Watch ' + pitch, seed: 21 });
  const c = p.addChannel({ instrument: {pluginId: plugin, pluginVersion: '1.0.0',parameters:{}} });
  p.addTrack('Keys').use(c).add(new Pattern({lengthBeats: 64, notes:[{pitch,start:0,duration:32,velocity:0.6}]})).at({bar:1});
  return p;
};`;

it("watches imported TS and swaps the native playing graph; failures and stale revisions retain the last project", async () => {
  await mkdir(cache, { recursive: true });
  const dir = await mkdtemp(join(cache, "preview-"));
  const ipcDir = await mkdtemp("/tmp/oxitone-preview-test-");
  const socketPath = join(ipcDir, "ipc");
  const binary = fileURLToPath(new URL("../../../target/debug/oxitone-preview", import.meta.url));
  const viewer = spawn(binary, ["--socket", socketPath, "--headless"], { stdio: ["ignore", "ignore", "pipe"] });
  let viewerLog = "";
  viewer.stderr.on("data", (chunk: Buffer) => {
    viewerLog += chunk.toString();
  });
  viewer.on("error", (error) => {
    viewerLog += error.message;
  });
  const entry = join(dir, "song.ts");
  const notes = join(dir, "notes.ts");
  const frames: PreviewFrame[] = [];
  const responses: Record<string, unknown>[] = [];
  let client: Client | undefined;
  let runner: PreviewRunner | undefined;
  let failure: unknown;
  const writeNotes = (pitch: number, plugin = "oxitone.wavetable") =>
    writeFile(notes, `export const pitch=${pitch}; export const plugin='${plugin}';`);
  try {
    await writeFile(join(dir, "package.json"), '{"type":"module"}');
    await writeFile(entry, source);
    await writeNotes(60);
    client = await Client.open(socketPath);
    runner = new PreviewRunner(
      entry,
      (frame) => {
        frames.push(frame);
        void client!
          .request(frame)
          .then((response) => {
            responses.push(previewResponseSchema.parse(response));
            if (response.type === "rejected" && frame.type === "snapshot")
              runner!.rejectRevision(frame.snapshot.revision);
          })
          .catch((error: Error) => {
            viewerLog += error.message;
          });
      },
      { debounceMs: 20 },
    );
    await runner.start();
    await until(async () => (await client!.query()).revision === "1");
    const first = frames.find((f) => f.type === "snapshot")!;
    expect(first.type).toBe("snapshot");
    const transport = (command: Record<string, unknown>) =>
      client!.request({ type: "transport", protocolVersion: "1.0", command });
    await transport({ command: "seek", frame: "48000" });
    expect((await client.query()).audibleFrame).toBe("48000");
    await transport({ command: "play" });
    await until(async () => BigInt(String((await client!.query()).cursor)) > 50_000n);
    await writeNotes(64);
    await until(async () => (await client!.query()).revision === "2");
    const before = await client.query();
    expect(before.playing).toBe(true);
    await writeNotes(65, "missing.instrument");
    await until(() => responses.some((r) => r.type === "rejected"));
    const rejected = await client.query();
    expect(rejected.revision).toBe("2");
    expect(rejected.playing).toBe(true);
    expect(BigInt(String(rejected.cursor))).toBeGreaterThan(BigInt(String(before.cursor)));
    await writeFile(
      notes,
      "export const pitch=65; export const plugin='missing.instrument'; // retry native validation",
    );
    await until(async () => (await client!.query()).seenRevision === "4");
    expect((await client.query()).revision).toBe("2");
    await writeFile(notes, "export const pitch = ;");
    await until(() => frames.some((f) => f.type === "diagnostic"));
    expect((await client.query()).revision).toBe("2");
    await writeNotes(67);
    await until(async () => (await client!.query()).revision === "5");
    const count = frames.filter((f) => f.type === "snapshot").length;
    const statusCount = frames.filter((f) => f.type === "status" && f.state === "watching").length;
    await writeFile(notes, "export const pitch=67; export const plugin='oxitone.wavetable'; // unchanged music");
    await until(() => frames.filter((f) => f.type === "status" && f.state === "watching").length > statusCount);
    expect(frames.filter((f) => f.type === "snapshot")).toHaveLength(count);
    expect((await client.request(first)).revision).toBe("5");
    expect((await client.request({ protocolVersion: "2.0", type: "query" })).type).toBe("rejected");
    await transport({ command: "play", frame: "48000", loopRegion: { startFrame: "48000", endFrame: "96000" } });
    await until(async () => {
      const state = await client!.query();
      return (
        state.playing === true && BigInt(String(state.cursor)) >= 48_000n && BigInt(String(state.cursor)) < 96_000n
      );
    });
    await transport({ command: "stop" });
    await until(async () => (await client!.query()).playing === false);
    expect((await client.query()).cursor).toBe("0");
  } catch (error) {
    failure = new Error(
      `${String(error)}\nFrames: ${JSON.stringify(frames.filter((f) => f.type !== "snapshot"))}\nViewer: ${viewerLog}`,
    );
  } finally {
    try {
      await runner?.close();
      if (client && !client.socket.destroyed) await client.request({ protocolVersion: "1.0", type: "shutdown" });
    } catch (error) {
      failure ??= error;
    } finally {
      client?.socket.destroy();
      viewer.kill();
      await rm(dir, { recursive: true, force: true });
      await rm(ipcDir, { recursive: true, force: true });
    }
  }
  if (failure !== undefined) throw failure;
}, 120_000);

it("bounds execution time and cancels obsolete factory generations", async () => {
  await mkdir(cache, { recursive: true });
  const dir = await mkdtemp(join(cache, "preview-timeout-"));
  const entry = join(dir, "song.ts");
  const frames: PreviewFrame[] = [];
  const runner = new PreviewRunner(entry, (frame) => frames.push(frame), { debounceMs: 10, timeoutMs: 8000 });
  try {
    await writeFile(join(dir, "package.json"), '{"type":"module"}');
    await writeFile(entry, "export default async () => { await new Promise(() => {}); };");
    await runner.start();
    await until(() => frames.some((f) => f.type === "diagnostic"));
    // Keep a timer alive so the unfinished async factory cannot exit on an empty event loop.
    await writeFile(entry, "setInterval(() => {}, 100); export default async () => { await new Promise(() => {}); };");
    await until(() => frames.some((f) => f.type === "diagnostic" && f.message.includes("timed out")));
    await writeFile(entry, "import { Project } from '@oxitone/core'; export default new Project({name:'Recovered'});");
    await until(() => frames.some((f) => f.type === "snapshot"));
    expect(frames.filter((f) => f.type === "snapshot")).toHaveLength(1);
    await writeFile(
      entry,
      "import { Project } from '@oxitone/core'; import {writeFileSync} from 'node:fs'; export default async () => { writeFileSync(new URL('./started',import.meta.url),'started'); await new Promise(r=>setTimeout(r,1500)); return new Project({name:'Obsolete'}); };",
    );
    await until(() => existsSync(join(dir, "started")));
    await writeFile(entry, "import { Project } from '@oxitone/core'; export default new Project({name:'Latest'});");
    await until(() => frames.some((f) => f.type === "snapshot" && f.snapshot.name === "Latest"));
    await delay(1800);
    expect(frames.filter((f) => f.type === "snapshot")).toHaveLength(2);
    expect(frames.some((f) => f.type === "snapshot" && f.snapshot.name === "Obsolete")).toBe(false);
  } finally {
    await runner.close();
    await rm(dir, { recursive: true, force: true });
  }
}, 60_000);

it("watches explicit runtime assets while preserving the entry's import.meta.url", async () => {
  await mkdir(cache, { recursive: true });
  const dir = await mkdtemp(join(cache, "preview-assets-"));
  const entry = join(dir, "song.ts");
  const data = join(dir, "name.txt");
  const frames: PreviewFrame[] = [];
  const runner = new PreviewRunner(entry, (frame) => frames.push(frame), { debounceMs: 10, watchPaths: [data] });
  try {
    await writeFile(join(dir, "package.json"), '{"type":"module"}');
    await writeFile(data, "First");
    await writeFile(
      entry,
      "import { Project } from '@oxitone/core'; import {readFileSync} from 'node:fs'; export default ()=>new Project({name:readFileSync(new URL('./name.txt',import.meta.url),'utf8')});",
    );
    await runner.start();
    await until(() => frames.some((f) => f.type === "snapshot" && f.snapshot.name === "First"));
    await writeFile(data, "Second");
    await until(() => frames.some((f) => f.type === "snapshot" && f.snapshot.name === "Second"));
    await writeFile(join(dir, "replacement.txt"), "Third");
    await rename(join(dir, "replacement.txt"), data);
    await until(() => frames.some((f) => f.type === "snapshot" && f.snapshot.name === "Third"));
  } finally {
    await runner.close();
    await rm(dir, { recursive: true, force: true });
  }
}, 30_000);
