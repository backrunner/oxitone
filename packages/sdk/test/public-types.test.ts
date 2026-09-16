import { expect, expectTypeOf, it } from "vitest";
import {
  Project,
  Pattern,
  createAutomationNamespace,
  wavetable,
  effect,
  type ArrangementEdit,
  type ProjectEdit,
  type ChanceOptions,
  type WaveKind,
  type Curve,
  type CurveKind,
  type InstrumentRef,
  type EffectRef,
  type CompileOptions,
  type RenderPosition,
  type PatternClip,
  type SampleClip,
  type Track,
} from "../src/index.js";

// Compile-time regressions: the package entry must describe the supported mutation path.
function forbiddenWrites(pattern: PatternClip, sample: SampleClip, track: Track) {
  // @ts-expect-error Relocation must also update membership and revision.
  pattern.track = track;
  // @ts-expect-error Direct timing writes bypass revision and restored wire state.
  pattern.startBeat = 4;
  // @ts-expect-error Sample membership is also read-only.
  sample.track = track;
}
void forbiddenWrites;

it("exports the named contracts needed to author reusable SDK helpers", () => {
  const instrument: InstrumentRef = wavetable();
  const insert: EffectRef = effect("delay");
  const chance: ChanceOptions = { seed: 1, probability: 0.5, intervalBeats: 1 };
  const wave: WaveKind = "sine";
  const curve: Curve = { kind: "linear" };
  const interpolation: CurveKind = curve.kind;
  const options: CompileOptions = { assetBaseDir: "." };
  const position: RenderPosition = { seconds: 1 };
  expectTypeOf(options).toMatchTypeOf<CompileOptions>();
  expectTypeOf(position).toMatchTypeOf<RenderPosition>();
  const project = new Project();
  const channel = project.addChannel({ instrument, effectChain: [insert] });
  project
    .addTrack()
    .use(channel)
    .add(new Pattern({ lengthBeats: 4, notes: [] }))
    .at({ bar: 1 });
  const config: ProjectEdit = { kind: "channel", index: 0, values: { level: 0.5 } };
  const placement: ArrangementEdit = { kind: "pattern", action: "move", resource: 0, clip: 0, track: 0, startBeat: 4 };
  project.configure(config).arrange(placement);
  expect(project.tracks[0]!.clips[0]!.startBeat).toBe(4);
  const automation = createAutomationNamespace();
  expect(automation.chance(chance).toSpec().kind).toBe("chance");
  expect(automation.wave(wave, { periodBeats: 1 }).toSpec().kind).toBe("wave");
  expect(automation.curve([{ beat: 0, value: 0, curve }], interpolation).toSpec().kind).toBe("curve");
});
