import { effectParameterSchemas, effectPluginIds, type EffectKind, type EffectParameters } from "@oxitone/protocol";
import { effectRefSchema, entityIdSchema, ErrorCode, OxitoneError, type EffectRef } from "@oxitone/protocol";
import { parseAuthoring } from "./authoring-validation.js";

export interface EffectOptions { mix?: number; bypass?: boolean }
/** Build a validated electronic effect. Parameters use physical units; automation uses normalized values. */
export function effect<K extends EffectKind>(kind: K, parameters?: EffectParameters<K>, options: EffectOptions = {}): EffectRef {
  if (!Object.hasOwn(effectParameterSchemas, kind)) throw new OxitoneError(ErrorCode.InvalidProject, "unknown built-in effect kind");
  const values = parseAuthoring<Record<string, number | undefined>>(effectParameterSchemas[kind], parameters === undefined ? {} : parameters, "effect.parameters");
  return parseAuthoring(effectRefSchema, {
    pluginId: effectPluginIds[kind], pluginVersion: "1.0.0",
    parameters: Object.fromEntries(Object.entries(values).filter(([, value]) => value !== undefined)),
    ...(options.mix === undefined ? {} : { mix: options.mix }),
    ...(options.bypass === undefined ? {} : { bypass: options.bypass }),
  }, "effect");
}

/** Project SampleRef ID, resolved and resampled before DSP. No filesystem access in process. */
export function convolver(impulseSampleId: string, parameters: EffectParameters<"convolver"> = {}, options: EffectOptions = {}): EffectRef {
  const id = parseAuthoring(entityIdSchema, impulseSampleId, "effect.resources.impulse");
  return { ...effect("convolver", parameters, options), resources: { impulse: id } };
}
