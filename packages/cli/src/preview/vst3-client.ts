import { connect } from "node:net";
import {
  ErrorCode,
  OxitoneError,
  previewFrameSchema,
  previewResponseSchema,
  type PreviewFrame,
  type PreviewResponse,
  type Vst3InstanceInventory,
  type Vst3InstanceRequest,
  type Vst3InstanceResult,
} from "@oxitone/protocol";
import { encodeFrame, FrameDecoder } from "./framing.js";

/** The document owner controls the native Preview graph, never a disposable validation engine. */
export interface Vst3Runtime {
  inventory(snapshotRevision: string, signal: AbortSignal): Promise<Vst3InstanceInventory>;
  control(snapshotRevision: string, request: Vst3InstanceRequest, signal: AbortSignal): Promise<Vst3InstanceResult>;
}

export class PreviewVst3Client implements Vst3Runtime {
  constructor(private readonly path: string) {}
  private async request(frame: PreviewFrame, signal: AbortSignal, timeoutMs: number): Promise<PreviewResponse> {
    if (signal.aborted) throw new OxitoneError(ErrorCode.SourceChanged, "VST3 document request was superseded");
    const bytes = encodeFrame(previewFrameSchema.parse(frame));
    return new Promise((resolve, reject) => {
      const socket = connect(this.path);
      const decoder = new FrameDecoder();
      let settled = false;
      const finish = (value: PreviewResponse | OxitoneError) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        signal.removeEventListener("abort", abort);
        socket.destroy();
        if (value instanceof OxitoneError) reject(value);
        else resolve(value);
      };
      const abort = () => finish(new OxitoneError(ErrorCode.SourceChanged, "VST3 document request was superseded"));
      const timer = setTimeout(
        () => finish(new OxitoneError(ErrorCode.PluginHostTimeout, "Preview VST3 control timed out")),
        timeoutMs,
      );
      signal.addEventListener("abort", abort, { once: true });
      socket.once("connect", () => socket.write(bytes));
      socket.once("error", (error) => finish(new OxitoneError(ErrorCode.PluginHostUnavailable, error.message)));
      socket.once("close", () =>
        finish(new OxitoneError(ErrorCode.PluginHostUnavailable, "Preview VST3 connection closed")),
      );
      socket.on("data", (chunk: Buffer) => {
        try {
          const values = decoder.push(chunk);
          if (values.length > 1) throw new Error("multiple replies to one VST3 request");
          if (values.length) finish(previewResponseSchema.parse(values[0]));
        } catch (error) {
          finish(new OxitoneError(ErrorCode.RealtimeFault, `Invalid Preview VST3 response: ${String(error)}`));
        }
      });
    });
  }
  private check(response: PreviewResponse, revision: string): void {
    if (response.type === "rejected") {
      const code = Object.values(ErrorCode).find((code) => code === response.code) ?? ErrorCode.RealtimeFault;
      throw new OxitoneError(code, response.message);
    }
    if (
      (response.type !== "vst3Instances" && response.type !== "vst3Control") ||
      response.snapshotRevision !== revision
    )
      throw new OxitoneError(ErrorCode.SourceChanged, "Preview VST3 response belongs to a different snapshot");
  }
  async inventory(snapshotRevision: string, signal: AbortSignal): Promise<Vst3InstanceInventory> {
    const response = await this.request(
      { protocolVersion: "1.0", type: "vst3Instances", snapshotRevision },
      signal,
      6000,
    );
    this.check(response, snapshotRevision);
    if (response.type !== "vst3Instances")
      throw new OxitoneError(ErrorCode.RealtimeFault, "Unexpected VST3 inventory response");
    return response.inventory;
  }
  async control(
    snapshotRevision: string,
    request: Vst3InstanceRequest,
    signal: AbortSignal,
  ): Promise<Vst3InstanceResult> {
    const response = await this.request(
      {
        protocolVersion: "1.0",
        type: "vst3Control",
        snapshotRevision,
        request: { ...request, timeoutMs: request.timeoutMs ?? 5000 },
      },
      signal,
      (request.timeoutMs ?? 5000) + 1000,
    );
    this.check(response, snapshotRevision);
    if (
      response.type !== "vst3Control" ||
      response.result.graphGeneration !== request.graphGeneration ||
      response.result.instanceId !== request.instanceId
    )
      throw new OxitoneError(ErrorCode.SourceChanged, "Preview VST3 response belongs to a different instance");
    return response.result;
  }
}
