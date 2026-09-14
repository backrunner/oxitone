import { writeFileSync } from "node:fs";
import { ErrorCode, ERROR_CODES, OxitoneError } from "@oxitone/protocol";

/** Private worker result channel, independent of user console/stderr output. */
export function writeEvaluationFailure(error: unknown): void {
  const code = OxitoneError.isOxitoneError(error) ? error.code : ErrorCode.DraftInvalid;
  const message = (error instanceof Error ? error.message : String(error)).slice(0, 16_384);
  try {
    writeFileSync(3, JSON.stringify({ evaluationProtocolVersion: 1, type: "error", error: { code, message } }));
  } catch {
    /* Parent retains bounded stderr as a fallback if the result pipe is closed. */
  }
}
export function readEvaluationFailure(bytes: Buffer): OxitoneError | undefined {
  try {
    const value = JSON.parse(bytes.toString("utf8"));
    if (
      value?.evaluationProtocolVersion === 1 &&
      value.type === "error" &&
      ERROR_CODES.includes(value.error?.code) &&
      typeof value.error.message === "string"
    ) {
      return new OxitoneError(value.error.code, value.error.message.slice(0, 16_384));
    }
  } catch {
    /* Crashes and arbitrary stderr do not carry structured authoring errors. */
  }
  return undefined;
}
