import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { chanceOptionsToWire } from "../src/authoring/automation-source.js";
import { beatToWire } from "../src/base/beat.js";
import { ErrorCode, ERROR_CODES, OxitoneError } from "../src/base/errors.js";
import { decodeProjectSnapshot, encodeProjectSnapshot, projectSnapshotSchema } from "../src/engine/snapshot.js";
import { checkProtocolVersion, PROTOCOL_VERSION } from "../src/base/version.js";

const fixtures = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "schemas", "fixtures");
const snapshotText = readFileSync(join(fixtures, "project-snapshot.canonical.json"), "utf8");

describe("protocol version", () => {
  it("exposes 1.2 and accepts the legacy minor", () => {
    expect(PROTOCOL_VERSION).toBe("1.2");
    expect(() => checkProtocolVersion("1.0")).not.toThrow();
    expect(() => checkProtocolVersion("1.1")).not.toThrow();
  });

  it("rejects unknown major versions and newer minors", () => {
    for (const version of ["2.0", "0.9", "1.3", "banana"]) {
      expect(() => checkProtocolVersion(version)).toThrowError(
        expect.objectContaining({ code: ErrorCode.ProtocolVersionUnsupported }) as Error,
      );
    }
  });
});

describe("project snapshot codec", () => {
  it("round-trips Track M/S and rejects them below their protocol floor", () => {
    const snapshot = decodeProjectSnapshot(snapshotText);
    snapshot.tracks[0]!.mute = true;
    snapshot.tracks[0]!.solo = false;
    expect(() => encodeProjectSnapshot(snapshot)).toThrowError(
      expect.objectContaining({ code: ErrorCode.ProtocolVersionUnsupported }),
    );
    snapshot.protocolVersion = PROTOCOL_VERSION;
    expect(decodeProjectSnapshot(encodeProjectSnapshot(snapshot))).toEqual(snapshot);
  });
  it("rejects instance semantics mislabeled with the legacy minor", () => {
    const snapshot = decodeProjectSnapshot(snapshotText);
    snapshot.channels[0]!.instrument.instanceId = "ins_test";
    expect(projectSnapshotSchema.safeParse(snapshot).success).toBe(false);
    for (const operation of [
      () => encodeProjectSnapshot(snapshot),
      () => decodeProjectSnapshot(JSON.stringify(snapshot)),
    ]) {
      expect(operation).toThrowError(expect.objectContaining({ code: ErrorCode.ProtocolVersionUnsupported }));
    }
    snapshot.protocolVersion = PROTOCOL_VERSION;
    expect(decodeProjectSnapshot(encodeProjectSnapshot(snapshot))).toEqual(snapshot);
    delete snapshot.channels[0]!.instrument.instanceId;
    snapshot.automation[0]!.target.scope = "plugin";
    snapshot.protocolVersion = "1.0";
    expect(projectSnapshotSchema.safeParse(snapshot).success).toBe(false);
  });
  it("decodes the canonical fixture and re-encodes byte-identically", () => {
    const snapshot = decodeProjectSnapshot(snapshotText);
    expect(encodeProjectSnapshot(snapshot)).toBe(snapshotText);
  });

  it("ignores unknown fields on read", () => {
    const withExtra = JSON.parse(snapshotText) as Record<string, unknown>;
    withExtra.futureField = { nested: [1, 2, 3] };
    const decoded = decodeProjectSnapshot(JSON.stringify(withExtra));
    expect("futureField" in decoded).toBe(false);
  });

  it("rejects an unknown major version", () => {
    const other = JSON.parse(snapshotText) as Record<string, unknown>;
    other.protocolVersion = "2.0";
    expect(() => decodeProjectSnapshot(JSON.stringify(other))).toThrow(OxitoneError);
  });

  it("rejects invalid snapshots with zod issues", () => {
    const broken = JSON.parse(snapshotText) as { tempoMap: unknown };
    broken.tempoMap = [];
    expect(() => projectSnapshotSchema.parse(broken)).toThrow();
  });
});

describe("chance options", () => {
  const toWire = (options: unknown) =>
    chanceOptionsToWire(options as Parameters<typeof chanceOptionsToWire>[0], beatToWire);

  it("serializes frequency as rate", () => {
    expect(toWire({ frequency: 2, probability: 0.5, seed: 1 })).toEqual({
      kind: "chance",
      rate: 2,
      probability: 0.5,
      seed: 1,
    });
  });

  it("keeps intervalBeats as a beat rational", () => {
    expect(toWire({ intervalBeats: 0.5, probability: 1, seed: 1 })).toEqual({
      kind: "chance",
      intervalBeats: { numerator: 1, denominator: 2 },
      probability: 1,
      seed: 1,
    });
  });

  it("rejects zero/multiple frequency selectors and bad values", () => {
    expect(() => toWire({ probability: 0.5, seed: 1 })).toThrow();
    expect(() => toWire({ rate: 2, frequency: 2, probability: 0.5, seed: 1 })).toThrow();
    expect(() => toWire({ rate: 0, probability: 0.5, seed: 1 })).toThrow();
    expect(() => toWire({ rate: Number.NaN, probability: 0.5, seed: 1 })).toThrow();
    expect(() => toWire({ rate: 2, probability: 1.5, seed: 1 })).toThrow();
  });
});

describe("error codes", () => {
  it("covers every stable code named in the docs", () => {
    const expected = [
      "InvalidProject",
      "TempoRange",
      "TempoMapOrder",
      "TempoMapComplexity",
      "EditTargetMissing",
      "EditTargetAmbiguous",
      "EditScopeConflict",
      "EditNotRepresentable",
      "SourceChanged",
      "DraftInvalid",
      "BudgetExceeded",
      "AutomationNonFinite",
      "AutomationRange",
      "AutomationPeriod",
      "AutomationChanceFrequency",
      "AutomationPoints",
      "AutomationExponentialZero",
      "AutomationDepthLimit",
      "AutomationNodeLimit",
      "AutomationRateBudget",
      "AutomationTempoRestriction",
      "TempoAutomationConflict",
      "AutomationTargetInvalid",
      "MidiChannelLimit",
      "SampleStretchRange",
      "SampleFormatUnsupported",
      "AssetUnavailable",
      "DeviceUnavailable",
      "RealtimeFault",
      "WavTooLarge",
      "PluginAbiMismatch",
      "PluginInstallFailed",
      "PluginManifestMismatch",
      "PluginMigrationFailed",
      "PluginTaskConflict",
      "PerformanceWarning",
    ];
    expect([...ERROR_CODES].sort()).toEqual([...expected, "ProtocolVersionUnsupported"].sort());
  });

  it("carries code/details/cause without relying on message", () => {
    const cause = new Error("root");
    const err = new OxitoneError(ErrorCode.AutomationPoints, "bad points", {
      details: { path: "$.automation[0].source.points" },
      cause,
    });
    expect(err.code).toBe("AutomationPoints");
    expect(err.details?.path).toBe("$.automation[0].source.points");
    expect(err.cause).toBe(cause);
    expect(OxitoneError.isOxitoneError(err)).toBe(true);
  });
});
