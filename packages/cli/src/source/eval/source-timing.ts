import { channel } from "node:diagnostics_channel";

const timing = channel("oxitone.source.timing");
/** Internal control-side profiling. No paths, source text or plugin values are published. */
export function sourceSpan(phase: string): () => void {
  if (!timing.hasSubscribers) return idle;
  const start = performance.now();
  return () => timing.publish({ phase, milliseconds: performance.now() - start });
}
const idle = () => {};
