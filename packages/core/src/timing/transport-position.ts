import {
  beatToWire,
  ErrorCode,
  frameToWire,
  OxitoneError,
  type ProjectSnapshot,
  type TransportCommand,
} from "@oxitone/protocol";
import { TimeSignatureMap, type BarBeatPosition } from "./time-signature.js";

/** Musical position, absolute seconds, or project-rate sample frames. Markers use stable IDs. */
export type TransportPosition =
  | BarBeatPosition
  | { beat: number }
  | { frame: bigint | number }
  | { frames: bigint | number }
  | { seconds: number }
  | { marker: string };

/** @internal Authoring positions use the last successfully compiled snapshot. */
export function positionFields(
  position: TransportPosition | undefined,
  snapshot: ProjectSnapshot,
): Pick<TransportCommand, "frame" | "beat" | "seconds"> {
  if (position === undefined) return {};
  try {
    const keys = Object.keys(position);
    const musical = "bar" in position && keys.every((key) => key === "bar" || key === "beat");
    if (!musical && keys.length !== 1) throw new Error("choose one transport position representation");
    if ("bar" in position) {
      const signatures = new TimeSignatureMap();
      const [first, ...rest] = snapshot.timeSignatureMap;
      if (first === undefined) throw new Error("time signature map is empty");
      signatures.set(first.numerator, first.denominator);
      for (const segment of rest) signatures.add(segment);
      return { beat: beatToWire(signatures.toBeats(position)) };
    }
    if ("marker" in position) {
      const marker = snapshot.markers.find((m) => m.id === position.marker);
      if (marker === undefined) throw new Error(`unknown marker: ${position.marker}`);
      return { beat: marker.startBeat };
    }
    if ("beat" in position) return { beat: beatToWire(position.beat) };
    if ("seconds" in position) {
      if (!Number.isFinite(position.seconds) || position.seconds < 0)
        throw new Error("seconds must be finite and non-negative");
      return { seconds: position.seconds };
    }
    const frame = "frame" in position ? position.frame : "frames" in position ? position.frames : undefined;
    if (frame === undefined || (typeof frame === "number" && !Number.isSafeInteger(frame))) {
      throw new Error("frame must be a safe integer or bigint");
    }
    return { frame: frameToWire(frame) };
  } catch (error) {
    if (error instanceof OxitoneError) throw error;
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      error instanceof Error ? error.message : "invalid transport position",
      {
        details: { path: "transport.position" },
      },
    );
  }
}
