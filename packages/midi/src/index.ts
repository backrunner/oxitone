import {
  encodeProjectSnapshot,
  midiExportOptionsSchema,
  type MidiExportOptions,
  type MidiExportReport,
  type ProjectSnapshot,
} from "@oxitone/protocol";
import { createEngine, dispose, exportMidi as nativeExportMidi } from "@oxitone/native";

export type { MidiExportOptions, MidiExportReport } from "@oxitone/protocol";

/** Anything that can produce a protocol snapshot (e.g. `@oxitone/core` `Project`). */
export interface SnapshotSource {
  snapshot(): ProjectSnapshot;
}

export type MidiExportSource = ProjectSnapshot | string | SnapshotSource;

function isSnapshotSource(source: MidiExportSource): source is SnapshotSource {
  return (
    typeof source === "object" && "snapshot" in source && typeof (source as SnapshotSource).snapshot === "function"
  );
}

/**
 * Export a project as an SMF Type 1 file through the native engine.
 *
 * `source` is a `@oxitone/core` `Project` (or any object with `snapshot()`),
 * a `ProjectSnapshot`, or an already-encoded snapshot JSON string. With
 * `options.path` the file is written by the native side and the report
 * carries `path`/`bytes`; otherwise the report carries `bytesBase64`.
 * The export is deterministic: the same snapshot, seed, and options always
 * produce identical bytes.
 */
export function exportMidi(source: MidiExportSource, options: MidiExportOptions): MidiExportReport {
  const validated = midiExportOptionsSchema.parse(options);
  const snapshot =
    typeof source === "string"
      ? source
      : isSnapshotSource(source)
        ? encodeProjectSnapshot(source.snapshot())
        : encodeProjectSnapshot(source);
  const engine = createEngine();
  try {
    return nativeExportMidi(engine, snapshot, validated);
  } finally {
    dispose(engine);
  }
}
