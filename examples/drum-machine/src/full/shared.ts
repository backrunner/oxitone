import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Pattern, createAutomationNamespace, type Project, type Track } from "@oxitone/core";
import { registerPluginOptionsSchema, type EffectRef } from "@oxitone/protocol";
import { drumPanel } from "../plugin-panels.js";
import { outputRoot } from "./paths.js";

export type Hit = { pitch: number; start: number; duration: number; velocity: number };
export type Section = readonly [name: string, startBar: number];
export const automation = createAutomationNamespace();
export const fx = (name: string, parameters: Record<string, number>, mix = 1): EffectRef =>
  ({ pluginId: `oxitone.${name}`, pluginVersion: "1.0.0", parameters, mix });
export const note = (pitch: number, start: number, duration = 0.2, velocity = 0.7): Hit =>
  ({ pitch, start, duration, velocity });
export function bar(track: Track, index: number, notes: Hit[], name: string): void {
  if (notes.length) track.add(new Pattern({ id: `pat_${track.id}_${index}`, name, lengthBeats: 4, notes })).at({ bar: index + 1 });
}
export function sections(project: Project, markers: readonly Section[], bars: number, gain: number): void {
  for (const [name, start] of markers) project.addMarker(name, start * 4);
  project.master.automate("level", automation.polyline([
    { beat: 0, value: 0 }, { beat: 2, value: gain / 2 },
    { beat: (bars - 2) * 4, value: gain / 2 }, { beat: bars * 4, value: 0 },
  ]));
}
/** Trust only the hash-pinned local drum library produced by the explicit prepare command. */
export function drumRegistration() {
  return registerPluginOptionsSchema.parse(JSON.parse(readFileSync(join(outputRoot, "drums.json"), "utf8")));
}
export function preview(project: Project): Project {
  project.registerPlugin(drumRegistration(), { allowPlugins: "any" });
  return project.registerPluginUi(drumPanel);
}
