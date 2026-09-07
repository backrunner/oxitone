import { WasmEngine } from "./wasm.js";
import { ACK, EPOCH, RUNNING, PcmRing } from "./ring.js";
import { PresentationCursor } from "./presentation.js";
import type { ProjectSnapshot } from "@oxitone/protocol";
import type { SampleFormat, WasmTransport } from "./types.js";
interface Request { id: number; type: string; [key: string]: unknown; }
const scope = globalThis as unknown as {
  onmessage: ((event: MessageEvent<Request>) => void) | null;
  postMessage(message: unknown): void;
};
let engine: WasmEngine | undefined, ring: PcmRing | undefined;
let sampleRate = 0, blockSize = 0, playing = false, primed = false;
const cursor = new PresentationCursor();
let serial = Promise.resolve();
let timer: ReturnType<typeof setTimeout> | undefined;

function audibleFrame(): bigint {
  return cursor.current(ring!);
}
function flush(at: bigint): void { ring!.flush(); cursor.reset(ring!, at); primed = false; }
function pump(): void {
  timer = setTimeout(pump, 4);
  if (!engine || !ring || !playing || Atomics.load(ring.header, ACK) !== Atomics.load(ring.header, EPOCH)) return;
  try {
    const target = Math.min(ring.frames, Math.max(blockSize, 2048));
    while (ring.buffered() + blockSize <= target) {
      const [left, right] = engine.process();
      if (!ring.write(left, right)) break;
    }
    if (!primed) { primed = true; Atomics.store(ring.header, RUNNING, 1); }
  } catch (error) {
    playing = false; Atomics.store(ring.header, RUNNING, 0);
    scope.postMessage({ type: "fault", error: { code: "RealtimeFault", message: String(error) } });
  }
}
async function handle(request: Request): Promise<unknown> {
  if (request.type === "init") {
    engine = await WasmEngine.create(request.wasmUrl as string);
    ring = new PcmRing(request.buffer as SharedArrayBuffer, request.frames as number);
    sampleRate = request.sampleRate as number;
    pump(); return null;
  }
  if (!engine || !ring) throw new Error("Worker is not initialized");
  switch (request.type) {
    case "importSample": return engine.importSample(request.bytes as Uint8Array, request.format as SampleFormat);
    case "compile": {
      const snapshot = request.snapshot as ProjectSnapshot;
      if (snapshot.sampleRate !== sampleRate) throw new Error("Project sampleRate must match AudioContext.sampleRate");
      if (snapshot.blockSize > Math.min(2048, ring.frames)) throw new Error("Project blockSize exceeds the Web Audio render horizon");
      const state = engine.compile(snapshot); // failure keeps previous graph and ring
      blockSize = state.blockSize;
      const at = audibleFrame();
      flush(at);
      engine.transport({ command: "seek", frame: at });
      return engine.state();
    }
    case "transport": {
      const options = request.options as WasmTransport;
      // Validate without disturbing buffered audio; rewind play/pause to audible position.
      const at = audibleFrame();
      const adjusted = options.command === "play" && options.frame === undefined ? { ...options, frame: at } : options;
      const state = engine.transport(adjusted);
      flush(options.command === "stop" ? 0n : options.frame === undefined ? at : BigInt(options.frame));
      playing = state.state === "playing";
      if (options.command === "pause") engine.transport({ command: "seek", frame: at });
      if (options.command === "play") cursor.loop = options.loop;
      if (options.command === "stop") cursor.loop = undefined;
      return engine.state();
    }
    case "setParameter":
      engine.setParameter(request.entityId as string, request.parameterId as string, request.value as number, request.atFrame as bigint | undefined);
      return null;
    case "state": return { ...engine.state(), cursor: audibleFrame().toString() };
    case "dispose":
      playing = false; flush(0n); if (timer) clearTimeout(timer); engine.dispose(); return null;
    default: throw new Error("Unknown Web Audio worker request");
  }
}
scope.onmessage = ({ data }) => {
  serial = serial.then(async () => {
    try { scope.postMessage({ id: data.id, ok: true, value: await handle(data) }); }
    catch (error) { scope.postMessage({ id: data.id, ok: false, error: {
      code: typeof error === "object" && error && "code" in error ? error.code : "InvalidProject",
      message: error instanceof Error ? error.message : String(error) } }); }
  });
};
