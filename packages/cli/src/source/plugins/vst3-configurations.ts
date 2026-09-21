import { ErrorCode, OxitoneError, type Vst3Configuration, type Vst3Info, type Vst3Preset } from "@oxitone/protocol";

/** Opaque state stays in Node; imported presets survive catalog refresh, verification does not. */
export class Vst3Configurations {
  private readonly active = new Map<string, Vst3Configuration>();
  private readonly imported = new Map<string, { configuration: Vst3Configuration; path?: string }>();
  get(handle: string): Vst3Configuration | undefined {
    return this.active.get(handle);
  }
  retained(handle: string): Vst3Configuration | undefined {
    return this.imported.get(handle)?.configuration;
  }
  path(handle: string): string | undefined {
    return this.imported.get(handle)?.path;
  }
  forgetInspection(handle: string): void {
    this.active.delete(handle);
  }
  refresh(): void {
    this.active.clear();
  }
  remove(handle: string): void {
    this.active.delete(handle);
    this.imported.delete(handle);
  }
  private budget(configuration: Vst3Configuration | null, handle: string, replacing: boolean): void {
    const retained = new Map<string, Set<Vst3Configuration>>();
    const keep = (id: string, value: Vst3Configuration) => {
      const values = retained.get(id) ?? new Set<Vst3Configuration>();
      values.add(value);
      retained.set(id, values);
    };
    if (configuration) keep(handle, configuration);
    for (const [id, value] of this.active) if (id !== handle) keep(id, value);
    for (const [id, value] of this.imported) if (!replacing || id !== handle) keep(id, value.configuration);
    const bytes = [...retained.values()].reduce(
      (sum, values) => sum + [...values].reduce((size, value) => size + value.stateBase64.length, 0),
      0,
    );
    if (bytes > 24 * 1024 * 1024)
      throw new OxitoneError(
        ErrorCode.BudgetExceeded,
        "VST3 configuration budget exceeded; remove entries to release presets",
      );
  }
  load(handle: string, preset: Vst3Preset, path: string): void {
    this.budget(preset.configuration, handle, true);
    this.active.delete(handle);
    this.imported.set(handle, { configuration: preset.configuration, path });
  }
  capture(handle: string, info: Vst3Info, origin: "inspection" | "editor" = "inspection"): Vst3Info {
    const retained = this.imported.get(handle)?.configuration;
    let configuration = info.configuration;
    if (
      retained &&
      configuration &&
      retained.classId === configuration.classId &&
      retained.sha256 === configuration.sha256 &&
      retained.stateBase64 === configuration.stateBase64 &&
      Object.keys(retained.parameters).length === Object.keys(configuration.parameters).length &&
      Object.entries(retained.parameters).every(([id, value]) => configuration!.parameters[id] === value)
    )
      configuration = retained;
    if (
      origin === "inspection" &&
      retained &&
      (retained.sha256 !== info.sha256 || retained.classId.toLowerCase() !== info.classId.toLowerCase())
    )
      throw new OxitoneError(ErrorCode.PluginManifestMismatch, "Preset requires the exact inspected VST3 bundle/class");
    if (configuration) {
      if (configuration.sha256 !== info.sha256 || configuration.classId.toLowerCase() !== info.classId.toLowerCase())
        throw new OxitoneError(
          ErrorCode.PluginManifestMismatch,
          "Preset requires the exact inspected VST3 bundle/class",
        );
      const parameters = new Map(info.parameters.map((parameter) => [parameter.id.toString(), parameter]));
      for (const id of Object.keys(configuration.parameters)) {
        if (!parameters.has(id) || parameters.get(id)!.readOnly)
          throw new OxitoneError(ErrorCode.PluginConfigInvalid, "Preset contains an unknown or read-only parameter");
      }
    }
    this.budget(configuration, handle, origin === "editor");
    if (origin === "editor" && configuration) this.imported.set(handle, { configuration });
    if (configuration) this.active.set(handle, configuration);
    return {
      ...info,
      configuration,
      parameters: info.parameters.map((p) => ({ ...p, value: configuration?.parameters[p.id] ?? p.value })),
    };
  }
}
