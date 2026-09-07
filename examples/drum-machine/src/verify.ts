import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Pattern, Project } from "@oxitone/core";
import { beatToWire, type ProjectSnapshot } from "@oxitone/protocol";
import { compile, getPluginDiagnostics, renderWav, setParameter, type EngineHandle } from "oxitone";
import { drumInstrument, referenceGain } from "./song.js";

export const hashFile = (path: string): string => createHash("sha256").update(readFileSync(path)).digest("hex");

/** Actual N-API -> Rust -> instrument dylib -> effect dylib -> WAV checks. */
export function verifyPlugins(engine: EngineHandle, output: string) {
  const project = new Project({ name: "Dynamic instrument/effect verification", seed: 42 });
  const channel = project.addChannel({ instrument: drumInstrument(), effectChain: [referenceGain], level: 0.4 });
  project.addTrack("All four pads").use(channel).add(new Pattern({ lengthBeats: 4,
    notes: [36, 38, 42, 46].map((pitch, i) => ({ pitch, start: i, duration: 0.1, velocity: 0.8 })) })).at({ bar: 1 });
  const input = project.snapshot();
  input.channels[0]!.instrument.parameters.volume = 0.8;
  const render = (name: string, snapshot: ProjectSnapshot = input, blockSize: 64 | 128 | 256 = 128) => {
    const report = renderWav(engine, snapshot, { path: join(output, `${name}.wav`),
      end: { beat: beatToWire(4) }, tailSeconds: 0.5, bitDepth: "float32", dither: "none", blockSize });
    return { ...report.files[0]!, sha256: hashFile(report.files[0]!.path) };
  };
  compile(engine, input);
  const unity = render("unity");
  assert(unity.peakDbfs > -40 && unity.truePeakDbfs < -1);
  const dry = structuredClone(input);
  dry.channels[0]!.effectChain = [];
  assert.equal(render("dry", dry).sha256, unity.sha256, "Dynamic unity effect changed PCM");
  const results: Record<string, number> = {};
  for (const [parameterId, initial] of [["volume", 0.8], ["insert.0.parameter.gain", 1]] as const) {
    compile(engine, input);
    setParameter(engine, channel.id, parameterId, initial / 2);
    const host = render(`${parameterId}-host`);
    results[parameterId] = unity.peakDbfs - host.peakDbfs;
    assert(Math.abs(results[parameterId]! - 6.0206) < 0.001, `${parameterId} did not halve the amplitude`);
    const assigned = structuredClone(input);
    if (parameterId === "volume") assigned.channels[0]!.instrument.parameters.volume = initial / 2;
    else assigned.channels[0]!.effectChain[0]!.parameters.gain = initial / 2;
    compile(engine, assigned);
    assert.equal(render(`${parameterId}-initial`, assigned).sha256, host.sha256);
    const automated = structuredClone(input);
    automated.automation.push({ id: "auto_verify", target: { entityId: channel.id, parameterId },
      source: { kind: "constant", value: parameterId === "volume" ? 0.4 : 0.25 } });
    compile(engine, automated);
    assert.equal(render(`${parameterId}-automation`, automated).sha256, host.sha256);
    // Automation wins over a competing host event and the snapshot's initial value.
    setParameter(engine, channel.id, parameterId, 0);
    assert.equal(render(`${parameterId}-priority`, automated).sha256, host.sha256);
  }
  compile(engine, input);
  for (const block of [64, 256] as const) assert.equal(render(`block-${block}`, input, block).sha256, unity.sha256);
  const short = structuredClone(input);
  short.channels[0]!.instrument.parameters.decay = 0.5;
  const long = structuredClone(input);
  long.channels[0]!.instrument.parameters.decay = 2;
  assert.notEqual(render("short-decay", short).sha256, render("long-decay", long).sha256);
  assert.throws(() => setParameter(engine, channel.id, "volume", 2));
  assert.throws(() => setParameter(engine, channel.id, "missing-parameter", 1));
  const diagnostics = getPluginDiagnostics(engine);
  assert.equal(diagnostics.length, 2);
  assert(diagnostics.every((plugin) => plugin.faults === 0));
  return { amplitudeReductionDb: results, blockSizeParity: [64, 128, 256], unity, diagnostics };
}
