import { expect, it } from "vitest";
import { pluginUiManifestSchema } from "../src/plugins/plugin-ui.js";
import { pluginUiFixture } from "../src/gen/plugin-ui.js";

it("validates native panel contracts and rejects executable or unbounded shapes", () => {
  expect(pluginUiManifestSchema.parse(pluginUiFixture)).toEqual(pluginUiFixture);
  for (const change of [{ uiVersion: "2.0" }, { size: { width: 10000, height: 500 } },
    { title: "x".repeat(65) }, { title: "Line\nbreak" }, { script: "arbitrary UI code" }, { pages: [] }]) {
    expect(pluginUiManifestSchema.safeParse({ ...pluginUiFixture, ...change }).success).toBe(false);
  }
  const layout = structuredClone(pluginUiFixture);
  layout.pages[0]!.groups[0]!.controls = Array.from({ length: 33 }, () => ({ kind: "knob", parameter: "pan" }));
  expect(pluginUiManifestSchema.safeParse(layout).success).toBe(false);
});
