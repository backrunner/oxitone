import { ErrorCode, OxitoneError, frameToWire, encodeProjectSnapshot, type ProjectSnapshot } from "@oxitone/protocol";

interface RequestSchema<T> {
  safeParse(
    input: unknown,
  ):
    | { success: true; data: T }
    | { success: false; error: { issues: readonly { path: PropertyKey[]; message: string }[] } };
}

/** Keep caller validation failures on the public error contract, before native execution. */
export function parseRequest<T>(schema: RequestSchema<T>, input: unknown, path: string): T {
  const result = schema.safeParse(input);
  if (result.success) return result.data;
  const issue = result.error.issues[0];
  throw new OxitoneError(ErrorCode.InvalidProject, issue?.message ?? "invalid native request", {
    details: { path: [path, ...(issue?.path ?? [])].join(".") },
  });
}

/** JSON inputs retain Rust's version-first validation; object inputs use the same error family. */
export function encodeRequestSnapshot(input: ProjectSnapshot | string): string {
  if (typeof input === "string") return input;
  try {
    return encodeProjectSnapshot(input);
  } catch (error) {
    if (error instanceof OxitoneError) throw error;
    throw new OxitoneError(ErrorCode.InvalidProject, "invalid project snapshot", {
      details: { path: "snapshot" },
      cause: error,
    });
  }
}

/** Numbers must be exact; bigint covers the full u64 frame range. */
export function parameterFrame(value: bigint | number | undefined): string | undefined {
  if (value === undefined) return undefined;
  try {
    if (typeof value !== "bigint" && !Number.isSafeInteger(value)) throw new Error("unsafe frame number");
    return frameToWire(value);
  } catch {
    throw new OxitoneError(ErrorCode.InvalidProject, "atFrame must be a safe integer or u64 bigint", {
      details: { path: "atFrame" },
    });
  }
}
