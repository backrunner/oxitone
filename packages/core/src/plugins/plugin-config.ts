import {
  effectRefSchema,
  instrumentRefSchema,
  ErrorCode,
  OxitoneError,
  type EffectRef,
  type InstrumentRef,
} from "@oxitone/protocol";
import { parseAuthoring } from "../authoring-validation.js";

/** Immutable authoring configuration. Construction never loads a library or creates a DSP instance. */
export class PluginConfig {
  declare readonly pluginId: string;
  declare readonly pluginVersion: string;
  declare readonly parameters: Readonly<Record<string, number>>;
  declare readonly resources?: Readonly<Record<string, string>>;
  declare readonly state?: unknown;
  declare readonly mix?: number;
  declare readonly bypass?: boolean;
  readonly #kind: "instrument" | "effect";

  constructor(kind: "instrument" | "effect", input: InstrumentRef | EffectRef) {
    if (kind !== "instrument" && kind !== "effect")
      throw new OxitoneError(ErrorCode.InvalidProject, "plugin configuration kind must be instrument or effect");
    this.#kind = kind;
    const schema =
      kind === "instrument"
        ? instrumentRefSchema.omit({ instanceId: true }).strict()
        : effectRefSchema.omit({ instanceId: true }).strict();
    const config = { ...input };
    delete config.instanceId;
    const spec = parseAuthoring(schema, config, "plugin.config");
    // Configuration must survive ordinary TS/JSON persistence, including structured builtin state.
    assertSerializable(spec, new Set(), 0);
    Object.assign(this, freeze(structuredClone(spec)));
    Object.freeze(this);
  }
  get kind(): "instrument" | "effect" {
    return this.#kind;
  }
  toSpec(): InstrumentRef | EffectRef {
    return structuredClone({ ...this });
  }
  withParameters(parameters: Readonly<Record<string, number>>): PluginConfig {
    return new PluginConfig(this.kind, { ...this.toSpec(), parameters: { ...this.parameters, ...parameters } });
  }
  withHost(settings: { mix?: number | undefined; bypass?: boolean | undefined }): PluginConfig {
    if (this.kind !== "effect")
      throw new OxitoneError(ErrorCode.InvalidProject, "only an effect has insert mix/bypass settings");
    return new PluginConfig("effect", {
      ...this.toSpec(),
      ...Object.fromEntries(Object.entries(settings).filter(([, value]) => value !== undefined)),
    });
  }
}

function assertSerializable(value: unknown, seen: Set<object>, depth: number): void {
  if (depth > 64) throw new OxitoneError(ErrorCode.BudgetExceeded, "plugin configuration exceeds depth 64");
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean" ||
    (typeof value === "number" && Number.isFinite(value))
  )
    return;
  if (
    typeof value !== "object" ||
    seen.has(value) ||
    (!Array.isArray(value) && ![Object.prototype, null].includes(Object.getPrototypeOf(value)))
  ) {
    throw new OxitoneError(ErrorCode.InvalidProject, "plugin configuration must contain finite JSON data");
  }
  seen.add(value);
  for (const child of Object.values(value)) assertSerializable(child, seen, depth + 1);
  seen.delete(value);
}
function freeze<T>(value: T): T {
  if (value && typeof value === "object") {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
}

/** Adapt a builtin helper or external factory result without copying its implementation. */
export function pluginConfig(kind: "instrument" | "effect", input: InstrumentRef | EffectRef): PluginConfig {
  if (input instanceof PluginConfig && input.kind === kind) return input;
  return new PluginConfig(kind, input);
}
