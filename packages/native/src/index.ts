import {
  decodeProjectSnapshot,
  encodeProjectSnapshot,
  engineDiagnosticsSchema,
  engineOptionsSchema,
  compileOptionsSchema,
  type CompileOptions,
  ErrorCode,
  frameToWire,
  registerPluginOptionsSchema,
  registeredPluginSchema,
  pluginDiagnosticsSchema,
  pluginInfoSchema,
  type PluginInfo,
  midiExportOptionsSchema,
  midiExportReportSchema,
  outputDeviceInfoSchema,
  outputLatencySchema,
  OxitoneError,
  renderOptionsSchema,
  renderReportSchema,
  transportCommandSchema,
  transportStateSchema,
  entityIdSchema,
  inspectSampleRequestSchema,
  sampleInfoSchema,
  cacheSampleRequestSchema,
  cachedSampleInfoSchema,
  checkProtocolVersion,
  PROTOCOL_VERSION,
  beatDurationQuerySchema,
  beatDurationResultSchema,
  beatToWire,
  type SampleInfo,
  type CachedSampleInfo,
  type RegisterPluginOptions,
  type RegisteredPlugin,
  type PluginDiagnostics,
  type EngineDiagnostics,
  type EngineOptions,
  type MidiExportOptions,
  type MidiExportReport,
  type OutputDeviceInfo,
  type OutputLatency,
  type ProjectSnapshot,
  type RenderOptions,
  type RenderReport,
  type TransportCommand,
  type TransportState,
} from "@oxitone/protocol";
import { call, native } from "./call.js";

export type {
  SampleInfo,
  CachedSampleInfo,
  RegisterPluginOptions,
  RegisteredPlugin,
  PluginManifest,
  PluginDiagnostics,
  PluginInfo,
  EngineDiagnostics,
  EngineOptions,
  MidiExportOptions,
  MidiExportReport,
  OutputDeviceInfo,
  OutputLatency,
  ProjectSnapshot,
  RenderOptions,
  RenderReport,
  TransportCommand,
  TransportState,
} from "@oxitone/protocol";

export interface EngineHandle {
  readonly id: string;
  readonly protocolVersion: string;
}

/** Synchronous duration conversion through Rust's effective tempo map; no engine needed. */
export function resolveBeatDuration(snapshot: ProjectSnapshot, startBeat: number, durationSeconds: number): number {
  const query = beatDurationQuerySchema.safeParse({ startBeat: beatToWire(startBeat), durationSeconds });
  if (!query.success) throw new OxitoneError(ErrorCode.InvalidProject, "invalid timing query", { details: { path: "durationSeconds" } });
  const result = beatDurationResultSchema.parse(JSON.parse(call((binding) => binding.resolveBeatDuration(encodeProjectSnapshot(snapshot), JSON.stringify(query.data)))));
  checkProtocolVersion(result.protocolVersion);
  return result.durationBeats.numerator / result.durationBeats.denominator;
}

/** Synchronous control-thread decode for metadata; never opens an audio device. */
export function inspectSample(path: string): SampleInfo {
  const request = inspectSampleRequestSchema.safeParse({ protocolVersion: PROTOCOL_VERSION, path });
  if (!request.success) {
    throw new OxitoneError(ErrorCode.InvalidProject, "invalid sample path", { details: { path: "path" } });
  }
  const response = call((binding) => binding.inspectSample(JSON.stringify(request.data)));
  const info = sampleInfoSchema.parse(JSON.parse(response));
  checkProtocolVersion(info.protocolVersion);
  return info;
}

/** Decode and atomically publish an immutable float32 WAV on the control thread. */
export function cacheSample(path: string, cacheDir: string): CachedSampleInfo {
  const request = cacheSampleRequestSchema.safeParse({ protocolVersion: PROTOCOL_VERSION, path, cacheDir });
  if (!request.success) {
    throw new OxitoneError(ErrorCode.InvalidProject, "invalid sample cache request", {
      details: { path: String(request.error.issues[0]?.path[0] ?? "path") },
    });
  }
  const response = call((binding) => binding.cacheSample(JSON.stringify(request.data)));
  const info = cachedSampleInfoSchema.parse(JSON.parse(response));
  checkProtocolVersion(info.protocolVersion);
  return info;
}

export function createEngine(options?: EngineOptions): EngineHandle {
  const validated = options === undefined ? undefined : engineOptionsSchema.parse(options);
  const json = call((binding) =>
    binding.createEngine(validated === undefined ? undefined : JSON.stringify(validated)),
  );
  const created: unknown = JSON.parse(json);
  if (typeof created !== "object" || created === null) {
    throw new OxitoneError(ErrorCode.RealtimeFault, "malformed createEngine response");
  }
  const { engineId, protocolVersion, audioBackend } = created as Record<string, unknown>;
  if (typeof engineId !== "string" || typeof protocolVersion !== "string") {
    throw new OxitoneError(ErrorCode.RealtimeFault, "malformed createEngine response");
  }
  if (options?.audioBackend === "simulated" && audioBackend !== "simulated") {
    call((binding) => binding.dispose(engineId));
    throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "native addon does not support simulated audio; rebuild it before testing");
  }
  return { id: engineId, protocolVersion };
}

/**
 * Load explicitly trusted native plugin code on the control thread.
 * The supplied manifest must match the library's C descriptor exactly.
 */
export function registerPlugin(engine: EngineHandle, options: RegisterPluginOptions): RegisteredPlugin {
  const validated = registerPluginOptionsSchema.parse(options);
  const result = call((binding) => binding.registerPlugin(engine.id, JSON.stringify(validated)));
  return registeredPluginSchema.parse(JSON.parse(result));
}

/** Fault counts per registered plugin; available before and during playback. */
export function getPluginDiagnostics(engine: EngineHandle): PluginDiagnostics[] {
  const result = call((binding) => binding.getPluginDiagnostics(engine.id));
  return pluginDiagnosticsSchema.array().parse(JSON.parse(result));
}

/** Authoritative descriptor metadata for a built-in or registered native plugin. */
export function getPluginInfo(engine: EngineHandle, pluginId: string, pluginVersion: string): PluginInfo {
  const result = call((binding) => binding.getPluginInfo(engine.id, pluginId, pluginVersion));
  const info = pluginInfoSchema.parse(JSON.parse(result));
  checkProtocolVersion(info.protocolVersion);
  return info;
}

export function compile(
  engine: EngineHandle,
  snapshot: ProjectSnapshot | string,
  options?: CompileOptions,
): ProjectSnapshot {
  const json = typeof snapshot === "string" ? snapshot : encodeProjectSnapshot(snapshot);
  const compiledOptions = compileOptionsSchema.parse(options ?? {});
  const echo = call((binding) => binding.compile(engine.id, json, JSON.stringify(compiledOptions)));
  return decodeProjectSnapshot(echo);
}

export function dispose(engine: EngineHandle): void {
  call((binding) => binding.dispose(engine.id));
}

/**
 * Export the snapshot as an SMF Type 1 file. With `options.path` the native
 * side writes the file and the report carries `path`/`bytes`; otherwise the
 * report carries `bytesBase64`. Diagnostics always include the channel
 * assignment and the tempo-event resolution used.
 */
export function exportMidi(
  engine: EngineHandle,
  snapshot: ProjectSnapshot | string,
  options: MidiExportOptions,
): MidiExportReport {
  const validated = midiExportOptionsSchema.parse(options);
  const json = typeof snapshot === "string" ? snapshot : encodeProjectSnapshot(snapshot);
  const report = call((binding) =>
    binding.exportMidi(engine.id, json, JSON.stringify(validated)),
  );
  return midiExportReportSchema.parse(JSON.parse(report));
}

export function getProtocolVersion(): string {
  return native().getProtocolVersion();
}

/**
 * Offline WAV export (04-api-contracts.md §RenderOptions/RenderReport).
 * `options.path` is the output file (`stems: "none"`) or output directory.
 * `options.assetBaseDir` resolves relative `SampleRef.assetUri` values.
 * Parameter events queued via {@link setParameter} on this engine are
 * applied to the render.
 */
export function renderWav(
  engine: EngineHandle,
  snapshot: ProjectSnapshot | string,
  options: RenderOptions,
): RenderReport {
  const validated = renderOptionsSchema.parse(options);
  const json = typeof snapshot === "string" ? snapshot : encodeProjectSnapshot(snapshot);
  const report = call((binding) => binding.renderWav(engine.id, json, JSON.stringify(validated)));
  return renderReportSchema.parse(JSON.parse(report));
}

/**
 * Advance the engine transport. The first `play` starts realtime output on
 * the configured CoreAudio device (engine options select device/rate/latency
 * policy); commands take effect at the ring horizon. Before the first
 * `play`, transport advances offline state only (no audio). `frame`/`beat`
 * select the target position for play/seek.
 */
export function enqueueTransport(engine: EngineHandle, command: TransportCommand): TransportState {
  const validated = transportCommandSchema.parse(command);
  const state = call((binding) =>
    binding.enqueueTransport(engine.id, JSON.stringify({ type: "transport", ...validated })),
  );
  return transportStateSchema.parse(JSON.parse(state));
}

/**
 * Queue a physical-value parameter event on the compiled graph
 * (control-thread enqueue; takes effect when the graph processes the
 * target frame — while playing, at the ring horizon).
 */
export function setParameter(
  engine: EngineHandle,
  entityId: string,
  parameterId: string,
  value: number,
  atFrame?: bigint | number,
): void {
  entityIdSchema.parse(entityId);
  if (!Number.isFinite(value)) {
    throw new OxitoneError(ErrorCode.AutomationRange, `parameter value must be finite, got ${value}`);
  }
  call((binding) =>
    binding.setParameter(
      engine.id,
      entityId,
      parameterId,
      value,
      atFrame === undefined ? undefined : frameToWire(atFrame),
    ),
  );
}

/** Audio output devices visible to CoreAudio, default device first. */
export function listOutputDevices(): OutputDeviceInfo[] {
  const devices = call((binding) => binding.listOutputDevices());
  return outputDeviceInfoSchema.array().parse(JSON.parse(devices));
}

/**
 * Ring horizon + device output latency breakdown for a playing engine
 * (project-rate frames + seconds). Fails with `DeviceUnavailable` until
 * the engine has started playback.
 */
export function getOutputLatency(engine: EngineHandle): OutputLatency {
  const latency = call((binding) => binding.getOutputLatency(engine.id));
  return outputLatencySchema.parse(JSON.parse(latency));
}

/**
 * Realtime diagnostics: counters (`blocks`, `deadlineMisses`, `xruns`,
 * `nanBlocks`, `queueDrops`), the `engineLoad` EMA, ring occupancy,
 * block-time percentiles, and the diagnostic events raised since the last
 * call. Fails with `DeviceUnavailable` until playback has started.
 */
export function getDiagnostics(engine: EngineHandle): EngineDiagnostics {
  const diagnostics = call((binding) => binding.getDiagnostics(engine.id));
  return engineDiagnosticsSchema.parse(JSON.parse(diagnostics));
}
