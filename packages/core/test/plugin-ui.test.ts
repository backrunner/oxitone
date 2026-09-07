import { expect, it } from "vitest";
import { Project, type PluginUiManifest } from "../src/index.js";

it("registers isolated version-specific Preview panels without changing musical state", () => {
  const project = new Project({ seed: 42 });
  const snapshot = project.snapshot();
  const layout: PluginUiManifest = { uiVersion: "1.0", pluginId: "oxitone.delay", pluginVersion: "1.0.0", title: "Echo",
    size: { width: 500, height: 300 }, pages: [{ id: "main", title: "Main", groups: [{ id: "delay", title: "Delay", columns: 1,
      controls: [{ kind: "knob", parameter: "feedback" }] }] }] };
  expect(project.registerPluginUi(layout)).toBe(project);
  layout.title = "Changed";
  expect(project.registeredPluginUis[0]!.title).toBe("Echo");
  project.registeredPluginUis[0]!.pages.length = 0;
  expect(project.registeredPluginUis[0]!.pages).toHaveLength(1);
  project.registerPluginUi(layout);
  expect(project.registeredPluginUis).toHaveLength(1);
  expect(project.registeredPluginUis[0]!.title).toBe("Changed");
  project.registerPluginUi({ ...layout, pluginVersion: "2.0.0" });
  expect(project.registeredPluginUis).toHaveLength(2);
  // Runtime-invalid layout metadata is carried to Preview for local recovery.
  project.registerPluginUi({ ...layout, pages: [] });
  expect(project.snapshot()).toEqual(snapshot);
  expect(project.registeredPlugins).toEqual([]);
});
