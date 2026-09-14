import { readFileSync, writeFileSync } from "node:fs";
import { createEngine, dispose, getPluginInfo, registerPlugin } from "@oxitone/native";
import { engineOptionsSchema, registerPluginOptionsSchema, OxitoneError, ErrorCode } from "@oxitone/protocol";

/** Validate only the selected library in a disposable helper. No DSP instances or device are created. */
try {
  const input = JSON.parse(readFileSync(process.argv[2]!, "utf8"));
  const registration = registerPluginOptionsSchema.parse(input.registration);
  const engine = createEngine(engineOptionsSchema.parse({ allowPlugins: input.allowPlugins ?? "signed-only" }));
  try {
    const registered = registerPlugin(engine, registration);
    writeFileSync(
      3,
      JSON.stringify({ registered, info: getPluginInfo(engine, registered.pluginId, registered.pluginVersion) }),
    );
  } finally {
    dispose(engine);
  }
} catch (error) {
  writeFileSync(
    3,
    JSON.stringify({
      error: {
        code: OxitoneError.isOxitoneError(error) ? error.code : ErrorCode.PluginManifestMismatch,
        message: String(error),
      },
    }),
  );
}
