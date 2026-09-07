#!/usr/bin/env node
import process from "node:process";
import {
  createEngine,
  dispose,
  exportMidi,
  listOutputDevices,
  renderWav,
} from "oxitone";
import { OxitoneError } from "@oxitone/protocol";
import { loadInput } from "./input.js";

function usage(): never {
  console.error("Usage: oxitone doctor | oxitone <render|export-midi> <snapshot.json|project-directory> <output>");
  process.exit(2);
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
      const project = await loadInput(input);
      const report = command === "render"
        ? renderWav(engine, project.snapshot, { path: output, assetBaseDir: project.assetBaseDir })
        : exportMidi(engine, project.snapshot, { path: output });
      console.log(JSON.stringify(report, null, 2));
    } finally {
      dispose(engine);
    }
    return;
  }
  usage();
}

main(process.argv.slice(2)).catch((error: unknown) => {
  console.error(JSON.stringify(OxitoneError.isOxitoneError(error)
    ? { code: error.code, message: error.message, details: error.details }
    : { message: error instanceof Error ? error.message : String(error) }));
  process.exitCode = 1;
});
