import { z } from "zod";
import {
  engineOptionsSchema,
  patternSourceDocumentSchema,
  documentViewSchema,
  documentRequestSchema,
  documentMessageSchema,
  compileOptionsSchema,
  projectFileSchema,
  pluginManifestSchema,
  pluginInstallManifestSchema,
  pluginUiManifestSchema,
  multisamplerStateSchema,
  pluginInfoSchema,
  presetSchema,
  previewFrameSchema,
  previewResponseSchema,
  registerPluginOptionsSchema,
  nativeCommandSchema,
  nativeEventSchema,
  projectSnapshotSchema,
  renderOptionsSchema,
  inspectSampleRequestSchema,
  sampleInfoSchema,
  cacheSampleRequestSchema,
  cachedSampleInfoSchema,
  slicerStateSchema,
  beatDurationQuerySchema,
  beatDurationResultSchema,
  vst3SourceSchema,
  vst3BundleSourceSchema,
  vst3ClassListSchema,
  vst3EditorOptionsSchema,
  vst3ConfigurationOptionsSchema,
  vst3EditorResultSchema,
  vst3ConfigurationSchema,
  vst3RenderOptionsSchema,
  vst3PresetSchema,
  vst3StreamStartSchema,
  vst3StreamReadySchema,
  vst3StreamScheduleSchema,
  vst3StreamManagerSchema,
  vst3ControlRequestSchema,
  vst3ControlResponseSchema,
  vst3EditPageSchema,
  vst3InstanceInventorySchema,
  vst3InstanceRequestSchema,
  vst3InstanceResultSchema,
  vst3TransportSchema,
  registerVst3OptionsSchema,
  registeredVst3Schema,
} from "../index.js";
import { write } from "./output.js";
import { pluginUiFixture } from "./plugin-ui.js";
import { automationRangeFixture } from "./automation-range.js";

function writeSchema(rel: string, schema: z.ZodType): void {
  const json = z.toJSONSchema(schema, { target: "draft-2020-12" });
  write(rel, `${JSON.stringify(json, null, 2)}\n`);
}

export function generateSchemas(): void {
  writeSchema("schemas/vst3-edit-page.schema.json", vst3EditPageSchema);
  writeSchema("schemas/vst3-instance-inventory.schema.json", vst3InstanceInventorySchema);
  writeSchema("schemas/vst3-instance-request.schema.json", vst3InstanceRequestSchema);
  writeSchema("schemas/vst3-instance-result.schema.json", vst3InstanceResultSchema);
  writeSchema("schemas/vst3-control-request.schema.json", vst3ControlRequestSchema);
  writeSchema("schemas/vst3-control-response.schema.json", vst3ControlResponseSchema);
  write("schemas/fixtures/automation-range.json", `${JSON.stringify(automationRangeFixture, null, 2)}\n`);
  writeSchema("schemas/pattern-source.schema.json", patternSourceDocumentSchema);
  writeSchema("schemas/document-view.schema.json", documentViewSchema);
  writeSchema("schemas/document-request.schema.json", documentRequestSchema);
  writeSchema("schemas/document-message.schema.json", documentMessageSchema);
  writeSchema("schemas/project-snapshot.schema.json", projectSnapshotSchema);
  writeSchema("schemas/project-file.schema.json", projectFileSchema);
  writeSchema("schemas/compile-options.schema.json", compileOptionsSchema);
  writeSchema("schemas/native-command.schema.json", nativeCommandSchema);
  writeSchema("schemas/native-event.schema.json", nativeEventSchema);
  writeSchema("schemas/engine-options.schema.json", engineOptionsSchema);
  writeSchema("schemas/render-options.schema.json", renderOptionsSchema);
  writeSchema("schemas/inspect-sample-request.schema.json", inspectSampleRequestSchema);
  writeSchema("schemas/sample-info.schema.json", sampleInfoSchema);
  writeSchema("schemas/cache-sample-request.schema.json", cacheSampleRequestSchema);
  writeSchema("schemas/cached-sample-info.schema.json", cachedSampleInfoSchema);
  writeSchema("schemas/slicer-state.schema.json", slicerStateSchema);
  writeSchema("schemas/beat-duration-query.schema.json", beatDurationQuerySchema);
  writeSchema("schemas/beat-duration-result.schema.json", beatDurationResultSchema);
  writeSchema("schemas/plugin-manifest.schema.json", pluginManifestSchema);
  writeSchema("schemas/plugin-install-manifest.schema.json", pluginInstallManifestSchema);
  writeSchema("schemas/plugin-ui.schema.json", pluginUiManifestSchema);
  writeSchema("schemas/multisampler-state.schema.json", multisamplerStateSchema);
  write(
    "schemas/fixtures/plugin-ui.json",
    `${JSON.stringify(pluginUiManifestSchema.parse(pluginUiFixture), null, 2)}\n`,
  );
  writeSchema("schemas/plugin-info.schema.json", pluginInfoSchema);
  writeSchema("schemas/preset.schema.json", presetSchema);
  writeSchema("schemas/preview-frame.schema.json", previewFrameSchema);
  writeSchema("schemas/preview-response.schema.json", previewResponseSchema);
  writeSchema("schemas/register-plugin-options.schema.json", registerPluginOptionsSchema);
  writeSchema("schemas/vst3-source.schema.json", vst3SourceSchema);
  writeSchema("schemas/vst3-bundle-source.schema.json", vst3BundleSourceSchema);
  writeSchema("schemas/vst3-class-list.schema.json", vst3ClassListSchema);
  writeSchema("schemas/vst3-editor-options.schema.json", vst3EditorOptionsSchema);
  writeSchema("schemas/vst3-configuration-options.schema.json", vst3ConfigurationOptionsSchema);
  writeSchema("schemas/vst3-editor-result.schema.json", vst3EditorResultSchema);
  writeSchema("schemas/vst3-configuration.schema.json", vst3ConfigurationSchema);
  writeSchema("schemas/vst3-render-options.schema.json", vst3RenderOptionsSchema);
  writeSchema("schemas/vst3-preset.schema.json", vst3PresetSchema);
  writeSchema("schemas/vst3-stream-start.schema.json", vst3StreamStartSchema);
  writeSchema("schemas/vst3-stream-ready.schema.json", vst3StreamReadySchema);
  writeSchema("schemas/vst3-stream-schedule.schema.json", vst3StreamScheduleSchema);
  writeSchema("schemas/vst3-stream-manager.schema.json", vst3StreamManagerSchema);
  writeSchema("schemas/vst3-transport.schema.json", vst3TransportSchema);
  writeSchema("schemas/vst3-registration.schema.json", registerVst3OptionsSchema);
  writeSchema("schemas/vst3-registered.schema.json", registeredVst3Schema);

  // The recursive automation source is hand-maintained: zod cannot emit a
  // self-referential JSON Schema from the lazy union.
  write("schemas/automation-source.schema.json", `${JSON.stringify(automationSourceJsonSchema(), null, 2)}\n`);
}

function automationSourceJsonSchema(): unknown {
  const unit = { type: "number", minimum: 0, maximum: 1 };
  return {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    title: "AutomationSourceSpec",
    $ref: "#/$defs/source",
    $defs: {
      beat: {
        type: "object",
        required: ["numerator", "denominator"],
        properties: {
          numerator: { type: "integer", minimum: 0 },
          denominator: { type: "integer", minimum: 1, maximum: 4294967295 },
        },
      },
      point: {
        type: "object",
        required: ["beat", "value"],
        properties: {
          beat: { $ref: "#/$defs/beat" },
          value: { type: "number" },
          curve: {
            oneOf: [
              {
                type: "object",
                required: ["kind"],
                properties: { kind: { enum: ["step", "linear", "smooth", "exponential"] } },
              },
              {
                type: "object",
                required: ["kind", "out", "in"],
                properties: {
                  kind: { const: "bezier" },
                  out: { type: "array", items: { type: "number" }, minItems: 2, maxItems: 2 },
                  in: { type: "array", items: { type: "number" }, minItems: 2, maxItems: 2 },
                },
              },
            ],
          },
        },
      },
      source: {
        oneOf: [
          {
            type: "object",
            required: ["kind", "base", "replacement", "startBeat", "endBeat"],
            properties: {
              kind: { const: "replaceRange" },
              base: { $ref: "#/$defs/source" },
              replacement: { $ref: "#/$defs/source" },
              startBeat: { $ref: "#/$defs/beat" },
              endBeat: { $ref: "#/$defs/beat" },
              fadeBeats: { $ref: "#/$defs/beat" },
            },
          },
          {
            type: "object",
            required: ["kind", "value"],
            properties: { kind: { const: "constant" }, value: { type: "number" } },
          },
          {
            type: "object",
            required: ["kind", "interpolation", "points"],
            properties: {
              kind: { const: "curve" },
              interpolation: { enum: ["step", "linear", "smooth", "exponential", "bezier"] },
              points: { type: "array", items: { $ref: "#/$defs/point" }, minItems: 1 },
            },
          },
          {
            type: "object",
            required: ["kind", "periodBeats", "duty"],
            properties: {
              kind: { const: "gate" },
              periodBeats: { $ref: "#/$defs/beat" },
              duty: unit,
              phase: { $ref: "#/$defs/beat" },
              on: unit,
              off: unit,
            },
          },
          {
            type: "object",
            required: ["kind", "probability", "seed", "rate"],
            properties: {
              kind: { const: "chance" },
              probability: unit,
              seed: { type: "integer", minimum: 0 },
              rate: { type: "number", exclusiveMinimum: 0 },
              smoothBeats: { $ref: "#/$defs/beat" },
              randomPhase: { enum: ["absolute", "restart"] },
            },
          },
          {
            type: "object",
            required: ["kind", "probability", "seed", "intervalBeats"],
            properties: {
              kind: { const: "chance" },
              probability: unit,
              seed: { type: "integer", minimum: 0 },
              intervalBeats: { $ref: "#/$defs/beat" },
              smoothBeats: { $ref: "#/$defs/beat" },
              randomPhase: { enum: ["absolute", "restart"] },
            },
          },
          {
            type: "object",
            required: ["kind", "wave", "periodBeats"],
            properties: {
              kind: { const: "wave" },
              wave: { enum: ["sine", "cos", "triangle", "saw", "ramp", "square"] },
              periodBeats: { $ref: "#/$defs/beat" },
              phase: { $ref: "#/$defs/beat" },
              min: unit,
              max: unit,
              pulseWidth: unit,
            },
          },
          {
            type: "object",
            required: ["kind", "input", "min", "max"],
            properties: {
              kind: { const: "map" },
              input: { $ref: "#/$defs/source" },
              min: unit,
              max: unit,
            },
          },
          {
            type: "object",
            required: ["kind", "op", "input"],
            properties: {
              kind: { const: "unary" },
              op: { enum: ["clamp", "invert", "quantize", "scale", "offset"] },
              input: { $ref: "#/$defs/source" },
              steps: { type: "integer", minimum: 2 },
              amount: { type: "number" },
              min: unit,
              max: unit,
            },
          },
          {
            type: "object",
            required: ["kind", "op", "left", "right"],
            properties: {
              kind: { const: "binary" },
              op: { enum: ["mix", "add", "multiply", "min", "max"] },
              left: { $ref: "#/$defs/source" },
              right: { $ref: "#/$defs/source" },
              amount: unit,
            },
          },
        ],
      },
    },
  };
}
