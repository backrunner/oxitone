/**
 * Stable error codes for the Oxitone protocol. Every code that appears in
 * `.agents/docs/` has an entry here; callers must branch on `code`, never on
 * the human-readable `message`.
 */
export const ErrorCode = {
  InvalidProject: "InvalidProject",
  EditTargetMissing: "EditTargetMissing",
  EditTargetAmbiguous: "EditTargetAmbiguous",
  EditScopeConflict: "EditScopeConflict",
  SourceChanged: "SourceChanged",
  EditNotRepresentable: "EditNotRepresentable",
  DraftInvalid: "DraftInvalid",
  BudgetExceeded: "BudgetExceeded",
  ProtocolVersionUnsupported: "ProtocolVersionUnsupported",
  TempoRange: "TempoRange",
  TempoMapOrder: "TempoMapOrder",
  TempoMapComplexity: "TempoMapComplexity",
  TempoAutomationConflict: "TempoAutomationConflict",
  AutomationNonFinite: "AutomationNonFinite",
  AutomationRange: "AutomationRange",
  AutomationPeriod: "AutomationPeriod",
  AutomationChanceFrequency: "AutomationChanceFrequency",
  AutomationPoints: "AutomationPoints",
  AutomationExponentialZero: "AutomationExponentialZero",
  AutomationDepthLimit: "AutomationDepthLimit",
  AutomationNodeLimit: "AutomationNodeLimit",
  AutomationRateBudget: "AutomationRateBudget",
  AutomationTempoRestriction: "AutomationTempoRestriction",
  AutomationTargetInvalid: "AutomationTargetInvalid",
  MidiChannelLimit: "MidiChannelLimit",
  SampleStretchRange: "SampleStretchRange",
  SampleFormatUnsupported: "SampleFormatUnsupported",
  AssetUnavailable: "AssetUnavailable",
  DeviceUnavailable: "DeviceUnavailable",
  RealtimeFault: "RealtimeFault",
  WavTooLarge: "WavTooLarge",
  PluginAbiMismatch: "PluginAbiMismatch",
  PluginManifestMismatch: "PluginManifestMismatch",
  PluginInstallFailed: "PluginInstallFailed",
  PluginTaskConflict: "PluginTaskConflict",
  PluginMigrationFailed: "PluginMigrationFailed",
  PerformanceWarning: "PerformanceWarning",
} as const;

export type OxitoneErrorCode = (typeof ErrorCode)[keyof typeof ErrorCode];

export const ERROR_CODES: readonly OxitoneErrorCode[] = Object.values(ErrorCode);

export interface OxitoneErrorDetails {
  readonly path?: string;
  readonly [key: string]: unknown;
}

/**
 * Structured error carried across the N-API boundary. `message` is for
 * humans only; programmatic handling must use `code` and `details.path`.
 */
export class OxitoneError extends Error {
  readonly code: OxitoneErrorCode;
  readonly details?: OxitoneErrorDetails;

  constructor(
    code: OxitoneErrorCode,
    message: string,
    options?: { details?: OxitoneErrorDetails; cause?: unknown },
  ) {
    super(message, options?.cause === undefined ? undefined : { cause: options.cause });
    this.name = "OxitoneError";
    this.code = code;
    if (options?.details !== undefined) {
      this.details = options.details;
    }
  }

  static isOxitoneError(value: unknown): value is OxitoneError {
    return value instanceof OxitoneError;
  }
}
