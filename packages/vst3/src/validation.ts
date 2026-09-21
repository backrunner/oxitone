import { ErrorCode, OxitoneError } from "@oxitone/protocol";

export function parse<T>(
  schema: { safeParse(value: unknown): { success: true; data: T } | { success: false; error: unknown } },
  value: unknown,
): T {
  const result = schema.safeParse(value);
  if (!result.success)
    throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Invalid VST3 contract", { cause: result.error });
  return result.data;
}
