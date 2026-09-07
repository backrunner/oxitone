import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { it, expect } from "vitest";
import { createEngine, registerPlugin, dispose } from "oxitone";
import { buildPlugins } from "../src/plugins.js";
import { verifyPlugins } from "../src/verify.js";

it("controls a real Rust drum cdylib chained into a C effect via N-API", () => {
  const output = mkdtempSync(join(tmpdir(), "oxitone-drums-"));
  const engine = createEngine({ allowPlugins: "any" });
  try {
    for (const options of buildPlugins(output)) registerPlugin(engine, options);
    expect(verifyPlugins(engine, output).diagnostics.every((plugin) => plugin.faults === 0)).toBe(true);
  } finally {
    dispose(engine);
    rmSync(output, { recursive: true, force: true });
  }
}, 120_000);
