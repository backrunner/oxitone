import { ErrorCode, OxitoneError } from "./errors.js";

/**
 * Canonical JSON encoding for Oxitone project files and protocol fixtures
 * (06-format-and-export.md §项目文件). Byte-identical output is required
 * from both the TypeScript and Rust encoders; the rules are:
 *
 * - Object keys are sorted lexicographically (all protocol keys are ASCII).
 * - Arrays whose elements are all objects carrying a string `id` field are
 *   sorted by that `id` ascending ("arrays ordered by stable ID"); every
 *   other array keeps its authoring order.
 * - Numbers must be finite. Integral numbers must be safe integers and are
 *   written without a fraction; other numbers use the ECMAScript shortest
 *   round-trip decimal (identical to `JSON.stringify`).
 * - `u64` frames/revisions travel as decimal strings upstream of this layer.
 * - Output is 2-space-indented JSON with LF line endings and a trailing LF.
 */

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeNumber(value: number): number {
  if (!Number.isFinite(value)) {
    throw new OxitoneError(ErrorCode.AutomationNonFinite, `non-finite number in canonical JSON: ${value}`);
  }
  if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      `integer ${value} exceeds the JSON safe-integer range; use a decimal string`,
    );
  }
  return value === 0 ? 0 : value;
}

/** Deep-apply the canonical ordering rules; returns a fresh structure. */
export function canonicalize(value: unknown): unknown {
  if (typeof value === "number") {
    return normalizeNumber(value);
  }
  if (typeof value === "bigint") {
    throw new OxitoneError(
      ErrorCode.InvalidProject,
      "bigint reached the canonical encoder; convert u64 values to decimal strings first",
    );
  }
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return value;
  }
  if (Array.isArray(value)) {
    const items = value.map(canonicalize);
    const sortable = items.length > 1 && items.every((item) => isPlainObject(item) && typeof item.id === "string");
    if (sortable) {
      items.sort((a, b) => {
        const ai = (a as { id: string }).id;
        const bi = (b as { id: string }).id;
        return ai < bi ? -1 : ai > bi ? 1 : 0;
      });
    }
    return items;
  }
  if (isPlainObject(value)) {
    const out: Record<string, unknown> = {};
    for (const key of Object.keys(value).sort()) {
      out[key] = canonicalize(value[key]);
    }
    return out;
  }
  throw new OxitoneError(ErrorCode.InvalidProject, `unsupported value in canonical JSON: ${typeof value}`);
}

/** Serialize a validated wire value to canonical JSON text (trailing LF). */
export function canonicalEncode(value: unknown): string {
  return `${JSON.stringify(canonicalize(value), null, 2)}\n`;
}
