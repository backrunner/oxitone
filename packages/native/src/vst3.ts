import {
  ErrorCode,
  OxitoneError,
  registerVst3OptionsSchema,
  registeredVst3Schema,
  vst3InstanceInventorySchema,
  vst3InstanceRequestSchema,
  vst3InstanceResultSchema,
  type RegisterVst3Options,
  type RegisteredVst3,
  type Vst3InstanceInventory,
  type Vst3InstanceRequest,
  type Vst3InstanceResult,
} from "@oxitone/protocol";
import { call, callAsync } from "./call.js";
import { parseRequest } from "./request.js";
import type { EngineHandle } from "./index.js";

/** Register an explicitly selected VST3 class in an isolated helper. */
export function registerVst3(engine: EngineHandle, options: RegisterVst3Options): RegisteredVst3 {
  const validated = parseRequest(registerVst3OptionsSchema, options, "vst3.registration");
  return registeredVst3Schema.parse(
    JSON.parse(call((binding) => binding.registerVst3(engine.id, JSON.stringify(validated)))),
  );
}

function unsupported(): never {
  throw new OxitoneError(ErrorCode.ProtocolVersionUnsupported, "native addon lacks VST3 instance control 1");
}

export function getVst3Instances(engine: EngineHandle): Vst3InstanceInventory {
  const result = call((binding) => {
    if (typeof binding.getVst3Instances !== "function") unsupported();
    return binding.getVst3Instances(engine.id);
  });
  return vst3InstanceInventorySchema.parse(JSON.parse(result));
}

/** Controls the accepted live instance without mutating the authoring snapshot. */
export async function controlVst3Instance(
  engine: EngineHandle,
  request: Vst3InstanceRequest,
): Promise<Vst3InstanceResult> {
  const validated = parseRequest(vst3InstanceRequestSchema, request, "vst3.instance");
  const json = await callAsync((binding) => {
    if (typeof binding.controlVst3Instance !== "function") unsupported();
    return binding.controlVst3Instance(engine.id, JSON.stringify(validated));
  });
  const result = vst3InstanceResultSchema.parse(JSON.parse(json));
  if (result.graphGeneration !== validated.graphGeneration || result.instanceId !== validated.instanceId) {
    throw new OxitoneError(ErrorCode.RealtimeFault, "VST3 control returned a different instance target");
  }
  return result;
}
