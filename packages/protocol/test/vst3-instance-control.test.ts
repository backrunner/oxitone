import { expect, it } from "vitest";
import { vst3InstanceInventorySchema, vst3InstanceRequestSchema } from "../src/index.js";

const request = {
  instanceControlVersion: 1,
  graphGeneration: "18446744073709551615",
  instanceId: "vst3_gain",
  command: { kind: "poll" },
};
it("accepts full u64 graph identity without converting it to a JS number", () => {
  expect(vst3InstanceRequestSchema.parse(request)).toEqual({ ...request, timeoutMs: 5000 });
  for (const graphGeneration of ["0", "01", "+1", "1.0", "1e2", "", "abc", "18446744073709551616", 1]) {
    expect(vst3InstanceRequestSchema.safeParse({ ...request, graphGeneration }).success).toBe(false);
  }
});
it("rejects invalid instance versions, identities, timeouts and hidden commands", () => {
  for (const change of [
    { instanceControlVersion: 2 },
    { instanceId: "" },
    { instanceId: "x".repeat(129) },
    { instanceId: "insert.0" },
    { timeoutMs: 0 },
    { timeoutMs: 600001 },
    { timeoutMs: 1.5 },
    { command: { kind: "poll", restore: {} } },
  ]) {
    expect(vst3InstanceRequestSchema.safeParse({ ...request, ...change }).success).toBe(false);
  }
  for (const state of ["prepared", "active", "retired"]) {
    expect(
      vst3InstanceInventorySchema.safeParse({
        instanceControlVersion: 1,
        graphGeneration: "1",
        state,
        instances: [],
      }).success,
    ).toBe(true);
  }
});
