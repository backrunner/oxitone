import { ErrorCode, OxitoneError } from "@oxitone/protocol";
export function resolve(..._paths: string[]): never {
  throw new OxitoneError(
    ErrorCode.AssetUnavailable,
    "Local filesystem paths require the Node host; upload sample bytes to WasmEngine in browsers",
  );
}
