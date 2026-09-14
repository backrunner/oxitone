import { ErrorCode, OxitoneError } from "./errors.js";

/** Current protocol version (`major.minor`). Major bumps are breaking. */
export const PROTOCOL_VERSION = "1.2";

export const PROTOCOL_MAJOR = 1;
export const PROTOCOL_MINOR = 2;

const VERSION_PATTERN = /^(\d+)\.(\d+)$/;

/**
 * Validate a `protocolVersion` string from a wire message. Unknown major
 * versions and minors newer than this implementation are rejected.
 */
export function checkProtocolVersion(version: string): void {
  const match = VERSION_PATTERN.exec(version);
  if (match === null) {
    throw new OxitoneError(
      ErrorCode.ProtocolVersionUnsupported,
      `malformed protocolVersion: ${JSON.stringify(version)}`,
    );
  }
  const major = Number(match[1]);
  const minor = Number(match[2]);
  if (major !== PROTOCOL_MAJOR || minor > PROTOCOL_MINOR) {
    throw new OxitoneError(
      ErrorCode.ProtocolVersionUnsupported,
      `unsupported protocolVersion ${version}; this build speaks ${PROTOCOL_VERSION}`,
    );
  }
}
