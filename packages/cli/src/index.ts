#!/usr/bin/env node
import process from "node:process";
import { createEngine, dispose, exportMidi, listOutputDevices, renderWav } from "@oxitone/native";
import { OxitoneError } from "@oxitone/protocol";
import { loadInput } from "./input.js";
import { launchPreview, parsePreviewArgs } from "./preview/launch.js";
import { bundleProject } from "./bundle.js";
import { resolve, parse } from "node:path";

function usage(): never {
  console.error(
    "Usage: oxitone doctor | oxitone <render|export-midi> <snapshot.json|project-directory> <output> | oxitone build <entry.ts> [-o project.mjs] [--watch] | oxitone <daw|preview> <entry.ts> [--no-watch] [--viewer path]",
  );
  process.exit(2);
}

async function main(argv: string[]): Promise<void> {
  const [command, input, output] = argv;
  if (!command) usage();
  if (command === "--help" || command === "-h") {
    console.log(
      "Oxitone: daw <entry.ts> [--no-watch] [--viewer path] · build <entry.ts> [-o project.mjs] [--watch] · preview <entry.ts|project.mjs> [--no-watch] [--watch-path path] [--viewer path] · render <project> <wav> · export-midi <project> <mid> · doctor",
    );
    return;
  }
  if (command === "build") {
    if (!input || input.startsWith("-")) usage();
    let destination = resolve(`${parse(input).name}.mjs`),
      watch = false;
    for (let i = 2; i < argv.length; i++) {
      if (argv[i] === "--watch") watch = true;
      else if (argv[i] === "-o" || argv[i] === "--out") {
        const path = argv[++i];
        if (!path || path.startsWith("-")) usage();
        destination = resolve(path);
      } else usage();
    }
    await bundleProject(input, destination, watch);
    return;
  }
  if (command === "preview" || command === "daw") {
    const { entry, options } = parsePreviewArgs(argv.slice(1));
    await launchPreview(entry, { ...options, edit: command === "daw" });
    return;
  }
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
      const report =
        command === "render"
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
  console.error(
    JSON.stringify(
      OxitoneError.isOxitoneError(error)
        ? { code: error.code, message: error.message, details: error.details }
        : { message: error instanceof Error ? error.message : String(error) },
    ),
  );
  process.exitCode = 1;
});
