import { toOxitoneError } from "./errors.js";
import { loadNativeBinding, type NativeBinding } from "./load.js";

let cachedBinding: NativeBinding | undefined;
export function native(): NativeBinding { return cachedBinding ??= loadNativeBinding(); }

export function call<T>(fn: (binding: NativeBinding) => T): T {
  try { return fn(native()); } catch (error) { throw toOxitoneError(error); }
}
