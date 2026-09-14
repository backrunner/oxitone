import { beatToWire, projectSnapshotSchema } from "@oxitone/protocol";
import type {
  SampleFormat,
  ProjectInput,
  WasmRenderOptions,
  WasmSampleInfo,
  WasmState,
  WasmTransport,
} from "./types.js";
import { WebRuntimeError } from "./errors.js";

interface Abi extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  oxi_abi_version(): number;
  oxi_alloc(length: number): number;
  oxi_free(pointer: number, length: number): void;
  oxi_command(pointer: number, length: number, data: number, dataLength: number): number;
  oxi_response_ptr(): number;
  oxi_response_len(): number;
  oxi_binary_ptr(): number;
  oxi_binary_len(): number;
  oxi_left_ptr(): number;
  oxi_right_ptr(): number;
  oxi_process(): number;
  oxi_allocations(): number;
  oxi_deallocations(): number;
}

export function snapshotOf(input: ProjectInput) {
  return projectSnapshotSchema.parse("snapshot" in input ? input.snapshot() : input);
}
export function frameString(value: bigint | number): string {
  if (typeof value === "number" && !Number.isSafeInteger(value))
    throw new WebRuntimeError("InvalidProject", "frame must be a safe integer or bigint");
  const frame = BigInt(value);
  // Reserve headroom for block advancement; JS control positions stay exactly representable.
  if (frame < 0n || frame > BigInt(Number.MAX_SAFE_INTEGER))
    throw new WebRuntimeError("InvalidProject", "frame must be between zero and 2^53-1");
  return frame.toString();
}

/** One import-free Rust engine per Wasm instance. All control methods are synchronous. */
export class WasmEngine {
  private readonly encoder = new TextEncoder();
  private readonly decoder = new TextDecoder();
  private abi: Abi | undefined;
  private blockSize = 0;
  private pcm: readonly [Float32Array, Float32Array] = [new Float32Array(), new Float32Array()];

  private constructor(instance: WebAssembly.Instance) {
    this.abi = instance.exports as Abi;
    if (this.abi.oxi_abi_version() !== 1)
      throw new WebRuntimeError("ProtocolVersionUnsupported", "unsupported Wasm ABI");
  }
  static async create(source: BufferSource | WebAssembly.Module | string | URL): Promise<WasmEngine> {
    const bytes =
      typeof source === "string" || source instanceof URL
        ? await (async () => {
            const response = await fetch(source);
            if (!response.ok) throw new WebRuntimeError("AssetUnavailable", `Wasm fetch failed: ${response.status}`);
            return response.arrayBuffer();
          })()
        : source;
    const module = bytes instanceof WebAssembly.Module ? bytes : await WebAssembly.compile(bytes);
    if (WebAssembly.Module.imports(module).length)
      throw new WebRuntimeError("PluginAbiMismatch", "Oxitone Wasm must have no host imports");
    return new WasmEngine(await WebAssembly.instantiate(module));
  }
  private requireAbi(): Abi {
    if (!this.abi) throw new WebRuntimeError("InvalidProject", "Wasm engine is disposed");
    return this.abi;
  }
  private refresh(): void {
    const abi = this.requireAbi();
    this.pcm = [
      new Float32Array(abi.memory.buffer, abi.oxi_left_ptr(), this.blockSize),
      new Float32Array(abi.memory.buffer, abi.oxi_right_ptr(), this.blockSize),
    ];
  }
  /** Versioned control ABI; public for non-browser runtime integrations. */
  command<T = unknown>(command: Record<string, unknown>, data: Uint8Array = new Uint8Array()): T {
    const abi = this.requireAbi();
    const json = this.encoder.encode(JSON.stringify({ ...command, protocolVersion: "1.0" }));
    if (json.length > 16 * 1024 * 1024 || data.length > 64 * 1024 * 1024)
      throw new WebRuntimeError("InvalidProject", "Wasm input exceeds byte limit");
    let ptr = 0,
      binary = 0;
    try {
      ptr = abi.oxi_alloc(json.length);
      if (!ptr) throw new WebRuntimeError("InvalidProject", "Wasm input allocation rejected");
      if (data.length) {
        binary = abi.oxi_alloc(data.length);
        if (!binary) throw new WebRuntimeError("InvalidProject", "Wasm asset allocation rejected");
      }
      new Uint8Array(abi.memory.buffer, ptr, json.length).set(json);
      if (data.length) new Uint8Array(abi.memory.buffer, binary, data.length).set(data);
      abi.oxi_command(ptr, json.length, binary, data.length);
      const response = JSON.parse(
        this.decoder.decode(new Uint8Array(abi.memory.buffer, abi.oxi_response_ptr(), abi.oxi_response_len())),
      ) as { ok: boolean; protocolVersion: string; value: T; error?: { code: string; message: string } };
      if (response.protocolVersion !== "1.0")
        throw new WebRuntimeError("ProtocolVersionUnsupported", "unsupported Wasm response");
      if (!response.ok) throw new WebRuntimeError(response.error!.code, response.error!.message);
      return response.value;
    } catch (error) {
      if (error instanceof WebAssembly.RuntimeError) {
        this.abi = undefined;
        throw new WebRuntimeError("RealtimeFault", `Wasm trapped; create a new engine: ${error.message}`);
      }
      throw error;
    } finally {
      if (this.abi) {
        if (ptr) abi.oxi_free(ptr, json.length);
        if (binary) abi.oxi_free(binary, data.length);
        // A compile can change block size even through the raw command API.
        if (command.type === "compile" && abi.oxi_left_ptr()) {
          const state = this.decoder.decode(
            new Uint8Array(abi.memory.buffer, abi.oxi_response_ptr(), abi.oxi_response_len()),
          );
          const result = JSON.parse(state) as { ok: boolean; value?: WasmState };
          if (result.ok) this.blockSize = result.value!.blockSize;
        }
        if (command.type === "dispose") this.blockSize = 0;
        this.refresh();
      }
    }
  }
  compile(input: ProjectInput): WasmState {
    return this.command({ type: "compile", snapshot: snapshotOf(input) });
  }
  importSample(bytes: Uint8Array, format: SampleFormat): WasmSampleInfo {
    return this.command({ type: "importSample", format }, bytes);
  }
  state(): WasmState {
    return this.command({ type: "state" });
  }
  transport(options: WasmTransport): WasmState {
    return this.command({
      type: "transport",
      command: options.command,
      ...(options.frame === undefined ? {} : { frame: frameString(options.frame) }),
      ...(options.loop === undefined
        ? {}
        : { loopRegion: [frameString(options.loop.startFrame), frameString(options.loop.endFrame)] }),
    });
  }
  setParameter(entityId: string, parameterId: string, value: number, atFrame?: bigint | number): void {
    this.command({
      type: "setParameter",
      entityId,
      parameterId,
      value,
      ...(atFrame === undefined ? {} : { atFrame: frameString(atFrame) }),
    });
  }
  resolveBeatDuration(input: ProjectInput, startBeat: number, durationSeconds: number): number {
    const beats = this.command<{ numerator: number; denominator: number }>({
      type: "resolveBeatDuration",
      snapshot: snapshotOf(input),
      startBeat: beatToWire(startBeat),
      durationSeconds,
    });
    return beats.numerator / beats.denominator;
  }
  /** Borrowed planar PCM, overwritten by the next process; invalidated by any control command. */
  process(): readonly [Float32Array, Float32Array] {
    const abi = this.requireAbi();
    if (abi.oxi_process() !== 0) throw new WebRuntimeError("RealtimeFault", "Wasm processor has no graph or faulted");
    return this.pcm;
  }
  private binary(): Uint8Array {
    const abi = this.requireAbi();
    return new Uint8Array(abi.memory.buffer, abi.oxi_binary_ptr(), abi.oxi_binary_len()).slice();
  }
  renderWav(options: WasmRenderOptions): Uint8Array {
    this.command({ ...options, type: "renderWav", startFrame: frameString(options.startFrame ?? 0) });
    return this.binary();
  }
  exportMidi(options: { ppq?: number; tempoEventResolutionTicks?: number } = {}): Uint8Array {
    this.command({ type: "exportMidi", options });
    return this.binary();
  }
  memoryDiagnostics() {
    const abi = this.requireAbi();
    return {
      bytes: abi.memory.buffer.byteLength,
      allocations: abi.oxi_allocations(),
      deallocations: abi.oxi_deallocations(),
    };
  }
  dispose(): void {
    if (!this.abi) return;
    this.command({ type: "dispose" });
    this.abi = undefined;
    this.pcm = [new Float32Array(), new Float32Array()];
  }
}
