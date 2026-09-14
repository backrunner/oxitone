import type { ProjectSnapshot, SampleRef } from "@oxitone/protocol";
export type SampleFormat = SampleRef["format"];

export interface SnapshotSource {
  snapshot(): ProjectSnapshot;
}
export type ProjectInput = ProjectSnapshot | SnapshotSource;
export interface AssetBytes {
  sample: SampleRef;
  bytes: Uint8Array;
}
export interface WasmState {
  protocolVersion: "1.0";
  sampleRate: number;
  blockSize: number;
  cursor: string;
  state: "stopped" | "playing" | "paused" | "rendering";
  latencyFrames: number;
  contentEndFrame: string;
  faulted: boolean;
}
export interface WasmTransport {
  command: "play" | "pause" | "stop" | "seek";
  frame?: bigint | number;
  loop?: { startFrame: bigint | number; endFrame: bigint | number };
}
export interface WasmRenderOptions {
  frames: number;
  startFrame?: bigint | number;
  bitDepth?: 16 | 24 | 32;
  dither?: boolean;
}
export interface WasmSampleInfo {
  sha256: string;
  format: SampleFormat;
  sampleRate: number;
  channels: 1 | 2;
  frames: string;
}
export interface WebAudioOptions {
  wasmUrl: string | URL;
  context?: AudioContext;
  workerUrl?: string | URL;
  workletUrl?: string | URL;
  ringFrames?: number;
}
export interface WebAudioDiagnostics {
  underruns: number;
  bufferedFrames: number;
  sampleRate: number;
  running: boolean;
}
