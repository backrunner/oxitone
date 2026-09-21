import { afterEach, expect, it } from "vitest";
import { mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { loadVst3Preset, parseVst3Preset, saveVst3Preset } from "../src/index.js";

const directories: string[] = [];
afterEach(async () => {
  await Promise.all(directories.splice(0).map((dir) => rm(dir, { recursive: true, force: true })));
});
async function file() {
  const dir = await mkdtemp(join(tmpdir(), "oxitone-vst3-preset-"));
  directories.push(dir);
  return join(dir, "preset.json");
}
const preset = () =>
  parseVst3Preset({
    formatVersion: 1,
    kind: "oxitone-vst3-preset",
    name: "Saved gain",
    source: { bundlePath: "/tmp/Gain.vst3", classId: "1".repeat(32) },
    configuration: {
      formatVersion: 1,
      classId: "1".repeat(32),
      sha256: "a".repeat(64),
      stateBase64: "AQID",
      parameters: { "9": 0.25 },
    },
  });

it("round trips opaque state and normalized parameters without loading a helper", async () => {
  const path = await file();
  await saveVst3Preset(path, preset());
  expect(await loadVst3Preset(path)).toEqual(preset());
  expect(await readFile(path, "utf8")).not.toContain("allowPlugins");
});
it("publishes once under concurrent saves and preserves existing recovery files", async () => {
  const path = await file();
  const saved = await Promise.allSettled([saveVst3Preset(path, preset()), saveVst3Preset(path, preset())]);
  expect(saved.filter((result) => result.status === "fulfilled")).toHaveLength(1);
  expect(await loadVst3Preset(path)).toEqual(preset());
  const future = JSON.stringify({ ...preset(), formatVersion: 2 });
  await writeFile(path, future);
  await expect(loadVst3Preset(path)).rejects.toMatchObject({ code: "ProtocolVersionUnsupported" });
  await expect(saveVst3Preset(path, preset())).rejects.toMatchObject({ code: "AssetUnavailable" });
  expect(await readFile(path, "utf8")).toBe(future);
  expect(await readdir(directories.at(-1)!)).toEqual(["preset.json"]);
});
it("rejects malformed identity, imported trust policy, oversized data and file symlinks", async () => {
  const value = preset();
  expect(() => parseVst3Preset({ ...value, source: { ...value.source, classId: "2".repeat(32) } })).toThrow();
  expect(() => parseVst3Preset({ ...value, source: { ...value.source, allowPlugins: "any" } })).toThrow();
  expect(() => parseVst3Preset({ ...value, source: { ...value.source, bundlePath: "relative.vst3" } })).toThrow();
  const path = await file();
  await writeFile(path, "x".repeat(8 * 1024 * 1024 + 1));
  await expect(loadVst3Preset(path)).rejects.toMatchObject({ code: "BudgetExceeded" });
  await writeFile(path, "corrupt {");
  await expect(loadVst3Preset(path)).rejects.toMatchObject({ code: "AssetUnavailable" });
  expect(await readFile(path, "utf8")).toBe("corrupt {");
  await symlink(path, `${path}.link`);
  await expect(loadVst3Preset(`${path}.link`)).rejects.toMatchObject({ code: "AssetUnavailable" });
});
