#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import process from "node:process";
import {
  createEngine,
  dispose,
  exportMidi,
  listOutputDevices,
  renderWav,
} from "oxitone";
import { decodeProjectSnapshot } from "@oxitone/protocol";

function usage(): never {
  console.error("Usage: oxitone <render|export-midi|doctor> ...");
  process.exit(2);
}

async function snapshot(path: string) {
  return decodeProjectSnapshot(await readFile(path, "utf8"));
}

async function main(argv: string[]): Promise<void> {
  const [command, input, output] = argv;
  if (!command) usage();
  if (command === "doctor") {
    const engine = createEngine();
    try {
      const devices = listOutputDevices();
      console.log(JSON.stringify({ protocolVersion: engine.protocolVersion, devices }, null, 2));
    } finally {
      dispose(engine);
    }
    return;
  }
  if ((command === "render" || command === "export-midi") && input && output) {
    const engine = createEngine();
    try {
      const project = await snapshot(input);
      const report = command === "render"
        ? renderWav(engine, project, { path: output })
        : exportMidi(engine, project, { path: output });
      console.log(JSON.stringify(report, null, 2));
    } finally {
      dispose(engine);
    }
    return;
  }
  usage();
}

main(process.argv.slice(2)).catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
