import { z } from "zod";
import { beatWireSchema } from "../base/beat.js";
import { entityIdSchema, frameWireSchema } from "../base/primitives.js";
import { projectSnapshotSchema } from "./snapshot.js";

/** Versioned command envelope sent from TypeScript to the native engine. */
export const nativeCommandSchema = z.union([
  z.object({
    type: z.literal("compile"),
    revision: frameWireSchema,
    snapshot: projectSnapshotSchema,
  }),
  z.object({
    type: z.literal("transport"),
    command: z.enum(["play", "pause", "stop", "seek"]),
    frame: frameWireSchema.optional(),
    beat: beatWireSchema.optional(),
    seconds: z.number().finite().nonnegative().optional(),
    loopRegion: z.object({ startFrame: frameWireSchema, endFrame: frameWireSchema }).optional(),
  }),
  z.object({
    type: z.literal("setParameter"),
    entityId: entityIdSchema,
    parameterId: z.string().min(1),
    value: z.number().finite(),
    atFrame: frameWireSchema.optional(),
  }),
]);
export type NativeCommand = z.infer<typeof nativeCommandSchema>;

/** Payload accepted by the native `enqueueTransport` command (the `transport` variant of {@link nativeCommandSchema}). */
export const transportCommandSchema = z.object({
  type: z.literal("transport").optional(),
  command: z.enum(["play", "pause", "stop", "seek"]),
  frame: frameWireSchema.optional(),
  beat: beatWireSchema.optional(),
  seconds: z.number().finite().nonnegative().optional(),
  loopRegion: z.object({ startFrame: frameWireSchema, endFrame: frameWireSchema }).optional(),
});
export type TransportCommand = z.infer<typeof transportCommandSchema>;

export const TRANSPORT_STATES = ["stopped", "playing", "paused", "rendering"] as const;

/** Transport state returned by the native `enqueueTransport` command. */
export const transportStateSchema = z.object({
  state: z.enum(TRANSPORT_STATES),
  cursor: frameWireSchema,
});
export type TransportState = z.infer<typeof transportStateSchema>;

export const NATIVE_EVENT_TYPES = ["compiled", "transport", "meter", "diagnostic", "fault"] as const;

/** Event envelope emitted by the native engine to TypeScript. */
export const nativeEventSchema = z.object({
  protocolVersion: z.string(),
  revision: frameWireSchema,
  type: z.enum(NATIVE_EVENT_TYPES),
  payload: z.unknown(),
});
export type NativeEvent = z.infer<typeof nativeEventSchema>;
