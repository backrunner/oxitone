import { z } from "zod";

/** Slots retain the VST3 bus index, including inactive auxiliary buses. */
export const vst3AudioBusSchema = z.strictObject({
  channels: z.number().int().min(1).max(2),
  active: z.boolean(),
});
export const vst3AudioBusesSchema = z.strictObject({
  inputs: z.array(vst3AudioBusSchema).max(16),
  outputs: z.array(vst3AudioBusSchema).max(16),
});
/** Explicit activation at initialization; lengths must match the inspected physical bus counts. */
export const vst3BusActivationSchema = z.strictObject({
  inputs: z.array(z.boolean()).max(16),
  outputs: z.array(z.boolean()).max(16),
});
export type Vst3AudioBuses = z.infer<typeof vst3AudioBusesSchema>;
export type Vst3BusActivation = z.infer<typeof vst3BusActivationSchema>;

/** Physical audio ports stay empty for MIDI-only processors; no synthetic audio bus. */
export const vst3AudioPortsShape = {
  inputChannels: z.number().int().min(0).max(2),
  outputChannels: z.number().int().min(0).max(2),
  audioBuses: vst3AudioBusesSchema,
  noteInput: z.boolean(),
  noteOutput: z.boolean(),
};
export function validVst3AudioPorts(info: {
  inputChannels: number;
  outputChannels: number;
  audioBuses: Vst3AudioBuses;
  noteOutput: boolean;
}): boolean {
  return (
    info.inputChannels === (info.audioBuses.inputs[0]?.channels ?? 0) &&
    info.outputChannels === (info.audioBuses.outputs[0]?.channels ?? 0) &&
    (info.audioBuses.outputs.length > 0 || (info.audioBuses.inputs.length === 0 && info.noteOutput))
  );
}
