import { access, readFile } from "node:fs/promises";
import { constants } from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join } from "node:path";
import { fileURLToPath } from "node:url";
import { ErrorCode, OxitoneError } from "@oxitone/protocol";

/** Resolve the explicit override, matching installed platform package, or local development build. */
export async function executable(options: { hostPath?: string }): Promise<string> {
  if (process.platform !== "darwin" || !["arm64", "x64"].includes(process.arch))
    throw new OxitoneError(ErrorCode.PluginCapabilityUnsupported, "VST3 helper supports macOS arm64/x64 only");
  const configured = options.hostPath ?? process.env.OXITONE_VST3_HOST_PATH;
  const candidates: string[] = [];
  if (configured !== undefined) candidates.push(configured);
  else {
    try {
      const packagePath = createRequire(import.meta.url).resolve(
        `@oxitone/vst3-host-darwin-${process.arch}/package.json`,
      );
      candidates.push(join(dirname(packagePath), "bin", "oxitone-vst3-host"));
    } catch {
      /* Optional platform dependency may have been omitted at installation. */
    }
    try {
      const metadata = JSON.parse(await readFile(new URL("../../../package.json", import.meta.url), "utf8"));
      if (metadata.name === "oxitone-workspace" && metadata.private === true) {
        candidates.push(
          fileURLToPath(new URL("../../../target/debug/oxitone-vst3-host", import.meta.url)),
          fileURLToPath(new URL("../../../target/release/oxitone-vst3-host", import.meta.url)),
        );
      }
    } catch {
      /* Never search a consumer's cwd or PATH for executable code. */
    }
  }
  for (const path of candidates) {
    if (!isAbsolute(path) || path.includes("\0"))
      throw new OxitoneError(ErrorCode.PluginHostUnavailable, "VST3 helper path must be absolute");
    try {
      await access(path, constants.X_OK);
      return path;
    } catch {
      /* Try the next unconfigured local candidate. Overrides never fall back. */
    }
  }
  throw new OxitoneError(
    ErrorCode.PluginHostUnavailable,
    `VST3 helper unavailable (${configured ?? candidates.join(", ")}); install @oxitone/vst3-host-darwin-${process.arch} at the SDK version or set OXITONE_VST3_HOST_PATH; developers can build with cargo build -p oxitone-vst3-host --features host,stream`,
  );
}
