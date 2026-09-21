import { compile, createEngine, dispose, registerPlugin, registerVst3 } from "@oxitone/native";
import { engineOptionsSchema, type PreviewSnapshotFrame } from "@oxitone/protocol";
import { sourceSpan } from "../eval/source-timing.js";

/** Control-side graph preparation only. Never starts a session or opens a system audio output. */
export function validateProjectFrame(frame: PreviewSnapshotFrame): void {
  const done = sourceSpan("native-validation");
  try {
    const engine = createEngine(
      engineOptionsSchema.parse({
        sampleRate: frame.snapshot.sampleRate,
        blockSize: frame.snapshot.blockSize,
        allowPlugins: frame.allowPlugins,
      }),
    );
    try {
      for (const plugin of frame.plugins) registerPlugin(engine, plugin);
      for (const plugin of frame.vst3Plugins) registerVst3(engine, plugin);
      compile(engine, frame.snapshot, { assetBaseDir: frame.assetBaseDir });
    } finally {
      dispose(engine);
    }
  } finally {
    done();
  }
}
