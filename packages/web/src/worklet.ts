import { PcmRing } from "./ring.js";
declare class AudioWorkletProcessor {
  constructor(options?: unknown);
}
declare function registerProcessor(name: string, processor: typeof AudioWorkletProcessor): void;

class OxitoneOutput extends AudioWorkletProcessor {
  private readonly ring: PcmRing;
  constructor(options: { processorOptions: { buffer: SharedArrayBuffer; frames: number } }) {
    super(options);
    this.ring = new PcmRing(options.processorOptions.buffer, options.processorOptions.frames);
  }
  process(_inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
    const output = outputs[0];
    if (output?.[0] && output[1]) this.ring.read(output[0], output[1]);
    return true;
  }
}
registerProcessor("oxitone-output", OxitoneOutput as unknown as typeof AudioWorkletProcessor);
