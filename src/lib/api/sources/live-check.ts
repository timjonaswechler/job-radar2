import { invoke } from "@tauri-apps/api/core";

import { decodeInstalledSource, type InstalledSource } from "./installed";
import type { Diagnostics, JsonObject } from "./profiles";

export type CheckReportKind = "source_live_check";
export type CheckReportSubjectType = "source";
export type CheckReportResult = "passed" | "failed";
export type CheckReportSubject = { type: CheckReportSubjectType; key: string };
export type CheckFingerprint = {
  kind: string;
  sha256?: string;
  reference?: string;
};
export type CheckReport = {
  schemaVersion: 1;
  kind: CheckReportKind;
  subject: CheckReportSubject;
  checkedAt: string;
  logicVersion: string;
  result: CheckReportResult;
  fingerprints: CheckFingerprint[];
  diagnostics: Diagnostics;
  details: JsonObject;
};
export type CheckReportFreshnessState = "fresh" | "stale";
export type CheckReportStaleReason =
  | "logic_version_changed"
  | "missing_report_fingerprint"
  | "changed_fingerprint_sha256"
  | "unexpected_report_fingerprint";
export type CheckReportStaleDetail = {
  kind: string;
  reference?: string;
  reason: CheckReportStaleReason;
  expectedSha256?: string;
  actualSha256?: string;
  expectedValue?: string;
  actualValue?: string;
};
export type CheckReportFreshness = {
  state: CheckReportFreshnessState;
  staleFingerprints: CheckReportStaleDetail[];
};
export type SourceLiveCheckReportStatus = {
  state: "fresh" | "stale" | "unknown";
  report: CheckReport | null;
  freshness: CheckReportFreshness | null;
};
export type SourceLiveCheckRunOutcome = { report: CheckReport };
export type SourceLiveCheckAdmissionOutcome =
  | { type: "checked"; report: CheckReport }
  | { type: "activated"; report: CheckReport; source: InstalledSource };
export type SourceLiveCheckErrorKind =
  | "invalid_key"
  | "not_found"
  | "built_in"
  | "invalid_lifecycle"
  | "stale_generation"
  | "limit_exceeded"
  | "storage"
  | "check";

export class SourceLiveCheckCommandError extends Error {
  constructor(
    public readonly kind: SourceLiveCheckErrorKind,
    message: string,
  ) {
    super(message);
    this.name = "SourceLiveCheckCommandError";
  }
}

export async function checkSource(sourceKey: string): Promise<SourceLiveCheckRunOutcome> {
  return decodeRunOutcome(await invokeLiveCheck("check_source", sourceKey));
}

export async function checkAndActivateSource(
  sourceKey: string,
): Promise<SourceLiveCheckAdmissionOutcome> {
  return decodeAdmissionOutcome(
    await invokeLiveCheck("check_and_activate_source", sourceKey),
  );
}

export async function getSourceLiveCheckReportStatus(
  sourceKey: string,
): Promise<SourceLiveCheckReportStatus> {
  return decodeSourceLiveCheckReportStatus(
    await invokeLiveCheck("get_source_live_check_report_status", sourceKey),
  );
}

async function invokeLiveCheck(command: string, sourceKey: string): Promise<unknown> {
  try {
    return await invoke<unknown>(command, { sourceKey });
  } catch (error) {
    throw decodeSourceLiveCheckError(error);
  }
}

export function decodeSourceLiveCheckError(error: unknown): Error {
  if (
    isRecord(error) &&
    [
      "invalid_key",
      "not_found",
      "built_in",
      "invalid_lifecycle",
      "stale_generation",
      "limit_exceeded",
      "storage",
      "check",
    ].includes(String(error.kind)) &&
    typeof error.message === "string"
  ) {
    return new SourceLiveCheckCommandError(
      error.kind as SourceLiveCheckErrorKind,
      error.message,
    );
  }
  return error instanceof Error ? error : new Error(String(error));
}

export function decodeRunOutcome(value: unknown): SourceLiveCheckRunOutcome {
  if (!isRecord(value)) invalid();
  return { report: decodeReport(value.report) };
}

export function decodeAdmissionOutcome(
  value: unknown,
): SourceLiveCheckAdmissionOutcome {
  if (!isRecord(value)) invalid();
  const report = decodeReport(value.report);
  if (value.type === "checked" && report.result === "failed") {
    return { type: "checked", report };
  }
  if (value.type === "activated" && report.result === "passed") {
    const source = decodeInstalledSource(value.source);
    if (source.document.status !== "active") return invalid();
    return { type: "activated", report, source };
  }
  return invalid();
}

export function decodeSourceLiveCheckReportStatus(
  value: unknown,
): SourceLiveCheckReportStatus {
  if (!isRecord(value) || !["fresh", "stale", "unknown"].includes(String(value.state))) {
    return invalid();
  }
  if (value.state === "unknown") {
    if (value.report != null || value.freshness != null) return invalid();
    return { state: "unknown", report: null, freshness: null };
  }
  const report = decodeReport(value.report);
  const freshness = decodeFreshness(value.freshness);
  if (freshness.state !== value.state) return invalid();
  return { state: value.state, report, freshness } as SourceLiveCheckReportStatus;
}

function decodeReport(value: unknown): CheckReport {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    value.kind !== "source_live_check" ||
    !isRecord(value.subject) ||
    value.subject.type !== "source" ||
    typeof value.subject.key !== "string" ||
    typeof value.checkedAt !== "string" ||
    typeof value.logicVersion !== "string" ||
    (value.result !== "passed" && value.result !== "failed") ||
    !Array.isArray(value.fingerprints) ||
    !Array.isArray(value.diagnostics) ||
    !isRecord(value.details)
  ) return invalid();
  for (const fingerprint of value.fingerprints) {
    if (
      !isRecord(fingerprint) ||
      typeof fingerprint.kind !== "string" ||
      (fingerprint.reference !== undefined && typeof fingerprint.reference !== "string") ||
      (fingerprint.sha256 !== undefined &&
        (typeof fingerprint.sha256 !== "string" ||
          !/^[a-fA-F0-9]{64}$/.test(fingerprint.sha256)))
    ) return invalid();
  }
  for (const diagnostic of value.diagnostics) {
    if (
      !isRecord(diagnostic) ||
      ![
        "schema",
        "registry",
        "compiler",
        "runtime",
        "detection",
        "source_validation",
      ].includes(String(diagnostic.category)) ||
      typeof diagnostic.code !== "string" ||
      typeof diagnostic.message !== "string" ||
      !["info", "warning", "error"].includes(String(diagnostic.severity)) ||
      typeof diagnostic.path !== "string" ||
      (diagnostic.strategyKey !== undefined &&
        typeof diagnostic.strategyKey !== "string") ||
      (diagnostic.details !== undefined && !isJsonValue(diagnostic.details))
    ) return invalid();
  }
  return value as CheckReport;
}

function decodeFreshness(value: unknown): CheckReportFreshness {
  if (
    !isRecord(value) ||
    (value.state !== "fresh" && value.state !== "stale") ||
    !Array.isArray(value.staleFingerprints)
  ) return invalid();
  if (value.state === "fresh" && value.staleFingerprints.length !== 0) return invalid();
  if (value.state === "stale" && value.staleFingerprints.length === 0) return invalid();
  for (const detail of value.staleFingerprints) {
    if (
      !isRecord(detail) ||
      typeof detail.kind !== "string" ||
      ![
        "logic_version_changed",
        "missing_report_fingerprint",
        "changed_fingerprint_sha256",
        "unexpected_report_fingerprint",
      ].includes(String(detail.reason)) ||
      [
        detail.reference,
        detail.expectedSha256,
        detail.actualSha256,
        detail.expectedValue,
        detail.actualValue,
      ].some((item) => item !== undefined && typeof item !== "string")
    ) return invalid();
  }
  return value as CheckReportFreshness;
}

function isJsonValue(value: unknown): boolean {
  if (value === null || ["string", "number", "boolean"].includes(typeof value)) return true;
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function invalid(): never {
  throw new Error("invalid Source Live Check transport");
}
