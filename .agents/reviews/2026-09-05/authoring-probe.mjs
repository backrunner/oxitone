// Run after pnpm build: node .agents/reviews/2026-09-05/authoring-probe.mjs
import assert from "node:assert/strict";
import { Project } from "../../../packages/core/dist/index.js";

const project = new Project();
const channel = project.addChannel();
const before = project.snapshot();
channel.level = 0.25;
channel.pan = 0.75;
const after = project.snapshot();
console.log(JSON.stringify({
  beforeRevision: before.revision,
  afterRevision: after.revision,
  beforeLevel: before.channels[0].level,
  afterLevel: after.channels[0].level,
  beforePan: before.channels[0].pan,
  afterPan: after.channels[0].pan,
}));
assert.ok(BigInt(after.revision) > BigInt(before.revision), "authoring mutation must bump revision");
