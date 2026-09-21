import {
  ErrorCode,
  OxitoneError,
  registerVst3OptionsSchema,
  type ProjectEdit,
  type RegisterPluginOptions,
} from "@oxitone/protocol";
import type { ProjectEvaluation } from "../eval/project-evaluation.js";
import type { ProjectPluginCatalog } from "./project-plugins.js";
import { appendProjectEdit, restoreProject } from "../editing/project-edit-writer.js";
import { resolveVst3Host } from "@oxitone/vst3";

export interface PluginAssignment {
  plugin: string;
  owner: string;
  target: "instrument" | "channelInsert" | "busInsert";
  instance?: string | undefined;
}
export async function preparePluginAssignment(
  before: ProjectEvaluation,
  files: ReadonlyMap<string, string>,
  catalog: ProjectPluginCatalog,
  input: PluginAssignment,
) {
  const selected = catalog.selection(input.plugin),
    definition = selected.entry;
  const kind = input.target === "instrument" ? "instrument" : "effect";
  if (definition.kind !== kind)
    throw new OxitoneError(ErrorCode.EditScopeConflict, "Plugin type does not match the selected slot");
  const project = restoreProject(before);
  const owners = input.target === "busInsert" ? project.mixerChannels : project.channels;
  const index = owners.findIndex((owner) => owner.id === input.owner);
  const owner = owners[index];
  if (!owner) throw new OxitoneError(ErrorCode.EditTargetMissing, "Plugin owner no longer exists");
  const slot =
    input.instance === undefined ? undefined : owner.effectInstances.findIndex((i) => i.id === input.instance);
  if (kind === "effect" && slot === -1)
    throw new OxitoneError(ErrorCode.EditTargetMissing, "Plugin instance no longer exists");
  const config = {
    pluginId: definition.pluginId,
    pluginVersion: definition.pluginVersion,
    parameters: {},
    ...(selected.vst3Info?.configuration ? { state: selected.vst3Info.configuration } : {}),
  };
  const edit: ProjectEdit =
    kind === "instrument"
      ? { kind, index, config }
      : {
          kind,
          owner: input.target === "busInsert" ? "bus" : "channel",
          index,
          ...(slot === undefined ? {} : { slot }),
          config,
        };
  project.configure(edit);
  let registration: RegisterPluginOptions | import("@oxitone/protocol").RegisterVst3Options | undefined;
  const registered = before.frame.plugins?.find(
    (p) => p.manifest.pluginId === definition.pluginId && p.manifest.pluginVersion === definition.pluginVersion,
  );
  if (definition.source === "vst3") {
    if (!selected.vst3Info || !selected.vst3Source || !definition.sha256)
      throw new OxitoneError(ErrorCode.PluginConfigInvalid, "VST3 inspection metadata is unavailable");
    if (
      !before.frame.vst3Plugins.some(
        (p) =>
          p.source.classId.toLowerCase() === selected.vst3Source!.classId.toLowerCase() &&
          p.source.expectedHash === definition.sha256,
      )
    ) {
      registration = {
        registrationVersion: 1,
        source: {
          ...selected.vst3Source,
          expectedHash: definition.sha256,
          allowPlugins: before.frame.allowPlugins ?? "signed-only",
        },
        metadata: { ...selected.vst3Info, configuration: null },
        helperPath: await resolveVst3Host({}),
      };
    }
  } else if (definition.source !== "builtin" && !registered) {
    if (!selected.registration || !definition.sha256)
      throw new OxitoneError(ErrorCode.PluginManifestMismatch, "Verified plugin registration is missing");
    registration = {
      ...selected.registration,
      libraryPath: selected.sourceLibraryPath ?? selected.registration.libraryPath,
      expectedHash: definition.sha256,
    };
  }
  if (registered && definition.sha256 !== registered.expectedHash)
    throw new OxitoneError(
      ErrorCode.PluginManifestMismatch,
      "This plugin version is already registered from a different library",
    );
  const written = appendProjectEdit(before, files, "configure", edit, registration);
  const expectedFrame = {
    ...before.frame,
    ...(written.registration ? { plugins: [...(before.frame.plugins ?? []), written.registration] } : {}),
    ...(written.vst3Registration
      ? { vst3Plugins: [...before.frame.vst3Plugins, registerVst3OptionsSchema.parse(written.vst3Registration)] }
      : {}),
  };
  return {
    before: { ...before, reads: [...before.reads, ...selected.reads] },
    expected: project.snapshot(),
    expectedFrame,
    files: written.files,
  };
}
