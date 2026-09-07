import type { SampleFormat } from "./types.js";
import { WebRuntimeError } from "./errors.js";
import { PcmRing, RUNNING, UNDERRUNS } from "./ring.js";
import { snapshotOf } from "./wasm.js";
import type { ProjectInput, WebAudioOptions, WasmState, WasmTransport, WasmSampleInfo, WebAudioDiagnostics } from "./types.js";

/** Worker-rendered Rust PCM, consumed by an allocation-free AudioWorklet adapter. */
export class WebAudioSession {
  readonly context: AudioContext;
  readonly output: AudioWorkletNode;
  private id = 0;
  private disposed = false;
  private failed: WebRuntimeError | undefined;
  private pending = new Map<number, { resolve(value: unknown): void; reject(error: unknown): void; timer: ReturnType<typeof setTimeout> }>();
  onFault: ((error: WebRuntimeError) => void) | undefined;
  private constructor(private worker: Worker, private ring: PcmRing,
    context: AudioContext, output: AudioWorkletNode, private ownsContext: boolean) {
    this.context = context; this.output = output;
    worker.onmessage = ({ data }: MessageEvent<{ id: number; type?: string; ok: boolean; value: unknown; error?: { code: string; message: string } }>) => {
      if (data.type === "fault") { this.fail(new WebRuntimeError(data.error!.code, data.error!.message)); return; }
      const pending = this.pending.get(data.id);
      if (!pending) return;
      this.pending.delete(data.id); clearTimeout(pending.timer);
      if (data.ok) pending.resolve(data.value);
      else pending.reject(new WebRuntimeError(data.error!.code, data.error!.message));
    };
    worker.onerror = (event) => this.fail(new WebRuntimeError("RealtimeFault", event.message || "Audio worker failed"));
    output.onprocessorerror = () => this.fail(new WebRuntimeError("RealtimeFault", "AudioWorklet failed"));
  }
  static async create(options: WebAudioOptions): Promise<WebAudioSession> {
    if (!globalThis.crossOriginIsolated || typeof SharedArrayBuffer === "undefined")
      throw new WebRuntimeError("DeviceUnavailable", "Web Audio requires a secure, cross-origin isolated page (COOP: same-origin; COEP: require-corp)");
    const ring = PcmRing.create(options.ringFrames ?? 4096);
    const context = options.context ?? new AudioContext({ latencyHint: "interactive" });
    let worker: Worker | undefined, output: AudioWorkletNode | undefined, session: WebAudioSession | undefined;
    try {
      await context.audioWorklet.addModule(options.workletUrl ?? new URL("./worklet.js", import.meta.url));
      output = new AudioWorkletNode(context, "oxitone-output", { numberOfInputs: 0, numberOfOutputs: 1,
        outputChannelCount: [2], processorOptions: { buffer: ring.buffer, frames: ring.frames } });
      worker = new Worker(options.workerUrl ?? new URL("./worker.js", import.meta.url), { type: "module" });
      session = new WebAudioSession(worker, ring, context, output, options.context === undefined);
      await session.request({ type: "init", wasmUrl: new URL(options.wasmUrl, location.href).href,
        buffer: ring.buffer, frames: ring.frames, sampleRate: context.sampleRate });
      output.connect(context.destination);
      return session;
    } catch (error) {
      if (session) await session.dispose();
      else { worker?.terminate(); output?.disconnect(); if (!options.context) await context.close(); }
      throw error;
    }
  }
  private fail(error: WebRuntimeError): void {
    this.failed = error; Atomics.store(this.ring.header, RUNNING, 0);
    this.worker.terminate(); this.output.disconnect();
    for (const item of this.pending.values()) { clearTimeout(item.timer); item.reject(error); }
    this.pending.clear(); this.onFault?.(error);
  }
  private request<T>(message: Record<string, unknown>): Promise<T> {
    if (this.disposed || this.failed) return Promise.reject(this.failed ?? new WebRuntimeError("InvalidProject", "Web Audio session is disposed"));
    const id = ++this.id;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.fail(new WebRuntimeError("RealtimeFault", "Audio worker request timed out"));
      }, 60000);
      this.pending.set(id, { resolve: (value) => resolve(value as T), reject, timer });
      try { this.worker.postMessage({ ...message, id }); }
      catch (error) { clearTimeout(timer); this.pending.delete(id); reject(error); }
    });
  }
  importSample(bytes: Uint8Array, format: SampleFormat): Promise<WasmSampleInfo> { return this.request({ type: "importSample", bytes, format }); }
  update(input: ProjectInput): Promise<WasmState> {
    try { return this.request({ type: "compile", snapshot: snapshotOf(input) }); }
    catch (error) { return Promise.reject(error); }
  }
  async play(frame?: bigint | number, loop?: WasmTransport["loop"]): Promise<WasmState> {
    await this.context.resume(); // invoke from a user gesture
    return this.request({ type: "transport", options: { command: "play", ...(frame === undefined ? {} : { frame }), ...(loop === undefined ? {} : { loop }) } });
  }
  pause(): Promise<WasmState> { return this.request({ type: "transport", options: { command: "pause" } }); }
  stop(): Promise<WasmState> { return this.request({ type: "transport", options: { command: "stop" } }); }
  seek(frame: bigint | number): Promise<WasmState> { return this.request({ type: "transport", options: { command: "seek", frame } }); }
  setParameter(entityId: string, parameterId: string, value: number, atFrame?: bigint | number): Promise<void> {
    return this.request({ type: "setParameter", entityId, parameterId, value, atFrame });
  }
  state(): Promise<WasmState> { return this.request({ type: "state" }); }
  diagnostics(): WebAudioDiagnostics {
    return { underruns: Atomics.load(this.ring.header, UNDERRUNS), bufferedFrames: this.ring.buffered(),
      sampleRate: this.context.sampleRate, running: !!Atomics.load(this.ring.header, RUNNING) };
  }
  async dispose(): Promise<void> {
    if (this.disposed) return;
    this.disposed = true;
    Atomics.store(this.ring.header, RUNNING, 0);
    this.worker.terminate(); this.output.disconnect();
    for (const item of this.pending.values()) { clearTimeout(item.timer); item.reject(new WebRuntimeError("InvalidProject", "Web Audio session disposed")); }
    this.pending.clear();
    if (this.ownsContext && this.context.state !== "closed") await this.context.close();
  }
}
