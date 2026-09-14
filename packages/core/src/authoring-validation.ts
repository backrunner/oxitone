import { ErrorCode, OxitoneError } from "@oxitone/protocol";

interface AuthoringSchema<T> {
  safeParse(
    input: unknown,
  ):
    | { success: true; data: T }
    | { success: false; error: { issues: readonly { path: PropertyKey[]; message: string }[] } };
}

/** Validate before mutation and detach nested caller-owned authoring values. */
export function parseAuthoring<T>(schema: AuthoringSchema<T>, input: unknown, path: string): T {
  const result = schema.safeParse(input);
  if (!result.success) {
    const issue = result.error.issues[0];
    throw new OxitoneError(ErrorCode.InvalidProject, issue?.message ?? "invalid authoring value", {
      details: { path: [path, ...(issue?.path ?? [])].join(".") },
    });
  }
  return structuredClone(result.data);
}
