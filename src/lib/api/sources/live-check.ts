import { invoke } from "@tauri-apps/api/core"
import type { Diagnostics, JsonObject } from "./profiles"

export type CheckReportKind = "source_live_check"

export type CheckReportSubjectType = "source"

export type CheckReportResult = "passed" | "failed"

export type CheckReportSubject = {
  type: CheckReportSubjectType
  key: string
}

export type CheckFingerprint = {
  kind: string
  sha256: string
  reference?: string
}

export type CheckReport = {
  schemaVersion: 1
  kind: CheckReportKind
  subject: CheckReportSubject
  checkedAt: string
  logicVersion: string
  result: CheckReportResult
  fingerprints: CheckFingerprint[]
  diagnostics: Diagnostics
  details: JsonObject
}

export type CheckReportFreshnessState = "fresh" | "stale"

export type CheckReportStaleReason =
  | "logic_version_changed"
  | "missing_report_fingerprint"
  | "changed_fingerprint_sha256"
  | "unexpected_report_fingerprint"

export type CheckReportStaleDetail = {
  kind: string
  reference?: string
  reason: CheckReportStaleReason
  expectedSha256?: string
  actualSha256?: string
  expectedValue?: string
  actualValue?: string
}

export type CheckReportFreshness = {
  state: CheckReportFreshnessState
  staleFingerprints: CheckReportStaleDetail[]
}

export type SourceLiveCheckReportStatus = {
  state: "fresh" | "stale" | "unknown"
  report?: CheckReport | null
  freshness?: CheckReportFreshness | null
}


export function checkSource(sourceKey: string) {
  return invoke<CheckReport>("check_source", { sourceKey })
}
export function checkAndActivateSource(sourceKey: string) {
  return invoke<CheckReport>("check_and_activate_source", { sourceKey })
}
export function checkAndReactivateSource(sourceKey: string) {
  return invoke<CheckReport>("check_and_reactivate_source", { sourceKey })
}
export function getSourceLiveCheckReportStatus(sourceKey: string) {
  return invoke<SourceLiveCheckReportStatus>("get_source_live_check_report_status", { sourceKey })
}
