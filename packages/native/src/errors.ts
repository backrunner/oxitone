import { ERROR_CODES, ErrorCode, OxitoneError, type OxitoneErrorCode } from "@oxitone/protocol";

interface WireError {
  code: OxitoneErrorCode;
  message: string;
  path?: string;
  details?: Record<string, unknown>;
}

function parseWireError(raw: string): WireError | undefined {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) {
      return undefined;
    }
    const { code, message, path, details } = parsed as Record<string, unknown>;
    if (typeof code !== "string" || typeof message !== "string") {
      return undefined;
    }
    if (!(ERROR_CODES as readonly string[]).includes(code)) {
      return undefined;
    }
    const wire: WireError = { code: code as OxitoneErrorCode, message };
    if (path !== undefined) {
      wire.path = String(path);
    }
    if (typeof details === "object" && details !== null) {
      wire.details = details as Record<string, unknown>;
    }
    return wire;
  } catch {
    return undefined;
  }
}

export function toOxitoneError(error: unknown): OxitoneError {
  if (error instanceof OxitoneError) {
    return error;
  }
  const raw = error instanceof Error ? error.message : String(error);
  const wire = parseWireError(raw);
  if (wire === undefined) {
    return new OxitoneError(ErrorCode.RealtimeFault, "native call failed without a structured error", { cause: error });
  }
  const details: Record<string, unknown> = { ...wire.details };
  if (wire.path !== undefined) {
    details.path = wire.path;
  }
  return new OxitoneError(wire.code, wire.message, {
    ...(Object.keys(details).length > 0 ? { details } : {}),
    cause: error,
  });
}
