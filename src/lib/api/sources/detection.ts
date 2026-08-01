import { invoke } from "@tauri-apps/api/core"
import type { Diagnostics, DetectionEvidenceKind, JsonObject, SupportLevel } from "./profiles"

export type SourceProposalEvidence = {
  kind: DetectionEvidenceKind
  message: string
  path?: string
  descriptorPath: string
}

export type SourceProposal = {
  profileKey: string
  profileName: string
  recommendedAccessPathKey: string
  recommendedAccessPathName: string
  sourceConfig: JsonObject
  keyCandidates: string[]
  nameCandidates: string[]
  captures: Record<string, string>
  evidence: SourceProposalEvidence[]
  supportLevel: SupportLevel
  provenance: DetectionProposalProvenance
}

export type DetectionOrigin = { strategyKey: string; schemaPath: string }
export type DetectionProposalProvenance = {
  captures: Record<string, DetectionOrigin[]>
  sourceConfig: Record<string, DetectionOrigin[]>
  recommendation: DetectionOrigin[]
  evidence: DetectionOrigin[][]
}

export type UnsupportedSourceProfile = {
  profileKey: string
  profileName: string
  supportLevel: SupportLevel
  captures: Record<string, string>
  evidence: SourceProposalEvidence[]
  provenance: DetectionProposalProvenance
}

export type SourceProposalDetectionStatus =
  | "matched"
  | "ambiguous"
  | "unsupported"
  | "failed"
  | "budget_exhausted"
  | "cancelled"

export type SourceProposalDetectionResult = {
  status: SourceProposalDetectionStatus
  proposals: SourceProposal[]
  unsupportedProfiles: UnsupportedSourceProfile[]
  diagnostics: Diagnostics
}

export type PhaseUsage = {
  strategyAttempts: number
  requests: number
  producedItems: number
  durationMs: number
  pages: number
  browserActions: number
  fanOut: number
  responseBytes: number
  browserRenderedBytes: number
}

export type PhaseExecutionReport = {
  usage: PhaseUsage
  completion:
    | { type: "accepted" }
    | { type: "policy_unsatisfied" }
    | { type: "execution_failed" }
    | { type: "cancelled"; reason: "user_cancelled" }
    | {
        type: "budget_exhausted"
        exhaustion: {
          dimension: string
          requested: number
          remaining: number
          limitSources: string[]
        }
      }
}

export type DetectionOutcome = SourceProposalDetectionResult & {
  profileDiagnostics: Diagnostics
  report: PhaseExecutionReport
}

export async function detectSourceProposalFromUrl(url: string): Promise<DetectionOutcome> {
  return decodeDetectionOutcome(
    await invoke<unknown>("detect_source_proposal_from_url", { url }),
  )
}

export function decodeDetectionOutcome(value: unknown): DetectionOutcome {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ["status", "proposals", "unsupportedProfiles", "profileDiagnostics", "diagnostics", "report"]) ||
    !isEnum(value.status, ["matched", "ambiguous", "unsupported", "failed", "budget_exhausted", "cancelled"]) ||
    !Array.isArray(value.proposals) ||
    !Array.isArray(value.unsupportedProfiles)
  ) {
    throw new Error("invalid Profile Detection transport")
  }
  const invalidCardinality =
    (value.status === "matched" && value.proposals.length !== 1) ||
    (value.status === "ambiguous" && value.proposals.length < 2) ||
    (value.status === "unsupported" && (value.proposals.length !== 0 || value.unsupportedProfiles.length === 0)) ||
    (["failed", "budget_exhausted", "cancelled"].includes(value.status) && value.proposals.length !== 0) ||
    (["budget_exhausted", "cancelled"].includes(value.status) && value.unsupportedProfiles.length !== 0)
  if (invalidCardinality) throw new Error("invalid Profile Detection outcome cardinality")
  value.proposals.forEach(decodeProposal)
  value.unsupportedProfiles.forEach(decodeUnsupportedProfile)
  decodeDiagnostics(value.profileDiagnostics, "Profile Detection installed-state diagnostics")
  decodeDiagnostics(value.diagnostics, "Profile Detection runtime diagnostics")
  decodeReport(value.report)
  return value as DetectionOutcome
}

function decodeProposal(value: unknown): void {
  if (
    !isRecord(value) ||
    typeof value.profileKey !== "string" ||
    typeof value.profileName !== "string" ||
    typeof value.recommendedAccessPathKey !== "string" ||
    typeof value.recommendedAccessPathName !== "string" ||
    !isJsonObject(value.sourceConfig) ||
    !isStringArray(value.keyCandidates) ||
    !isStringArray(value.nameCandidates) ||
    !isStringRecord(value.captures) ||
    !Array.isArray(value.evidence) ||
    !isSupportLevel(value.supportLevel)
  ) {
    throw new Error("invalid Profile Detection proposal")
  }
  value.evidence.forEach(decodeEvidence)
  decodeProvenance(value.provenance)
}

function decodeUnsupportedProfile(value: unknown): void {
  if (
    !isRecord(value) ||
    typeof value.profileKey !== "string" ||
    typeof value.profileName !== "string" ||
    !isSupportLevel(value.supportLevel) ||
    !isStringRecord(value.captures) ||
    !Array.isArray(value.evidence)
  ) {
    throw new Error("invalid unsupported Profile Detection result")
  }
  value.evidence.forEach(decodeEvidence)
  decodeProvenance(value.provenance)
}

function decodeEvidence(value: unknown): void {
  if (
    !isRecord(value) ||
    !isEnum(value.kind, ["url", "http", "html", "browser"]) ||
    typeof value.message !== "string" ||
    typeof value.descriptorPath !== "string" ||
    (value.path !== undefined && typeof value.path !== "string")
  ) {
    throw new Error("invalid Profile Detection evidence")
  }
}

function decodeProvenance(value: unknown): void {
  if (
    !isRecord(value) ||
    !isOriginRecord(value.captures) ||
    !isOriginRecord(value.sourceConfig) ||
    !isOrigins(value.recommendation) ||
    !Array.isArray(value.evidence) ||
    value.evidence.some((origins) => !isOrigins(origins))
  ) {
    throw new Error("invalid Profile Detection provenance")
  }
}

function decodeReport(value: unknown): void {
  if (!isRecord(value) || !isRecord(value.usage) || !isRecord(value.completion)) {
    throw new Error("invalid Profile Detection execution report")
  }
  const usage = value.usage
  const usageKeys = [
    "strategyAttempts", "requests", "producedItems", "durationMs", "pages",
    "browserActions", "fanOut", "responseBytes", "browserRenderedBytes",
  ]
  if (usageKeys.some((key) => !isNonNegativeInteger(usage[key]))) {
    throw new Error("invalid Profile Detection usage")
  }
  const completion = value.completion
  if (isEnum(completion.type, ["accepted", "policy_unsatisfied", "execution_failed"])) return
  if (completion.type === "cancelled" && completion.reason === "user_cancelled") return
  if (
    completion.type === "budget_exhausted" &&
    isRecord(completion.exhaustion) &&
    isEnum(completion.exhaustion.dimension, [
      "strategy_attempts", "requests", "produced_items", "duration", "pages",
      "browser_actions", "fan_out", "response_bytes", "browser_rendered_bytes", "logical_waits",
    ]) &&
    isNonNegativeInteger(completion.exhaustion.requested) &&
    isNonNegativeInteger(completion.exhaustion.remaining) &&
    isEnumArray(completion.exhaustion.limitSources, ["backend", "compiled", "caller"])
  ) return
  throw new Error("invalid Profile Detection completion")
}

function decodeDiagnostics(value: unknown, label: string): asserts value is Diagnostics {
  if (!Array.isArray(value)) throw new Error(`invalid ${label}`)
  for (const diagnostic of value) {
    if (
      !isRecord(diagnostic) ||
      !isEnum(diagnostic.category, ["schema", "registry", "compiler", "runtime", "detection", "source_validation"]) ||
      typeof diagnostic.code !== "string" ||
      typeof diagnostic.message !== "string" ||
      !isEnum(diagnostic.severity, ["info", "warning", "error"]) ||
      typeof diagnostic.path !== "string" ||
      (diagnostic.strategyKey !== undefined && typeof diagnostic.strategyKey !== "string") ||
      (diagnostic.details !== undefined && !isJsonValue(diagnostic.details))
    ) throw new Error(`invalid ${label}`)
  }
}

function isOrigin(value: unknown): boolean {
  return isRecord(value) && typeof value.strategyKey === "string" && typeof value.schemaPath === "string"
}

function isOrigins(value: unknown): boolean {
  return Array.isArray(value) && value.every(isOrigin)
}

function isOriginRecord(value: unknown): boolean {
  return isRecord(value) && Object.values(value).every(isOrigins)
}

function isSupportLevel(value: unknown): value is SupportLevel {
  return isEnum(value, ["stable", "best_effort", "experimental", "unsupported"])
}

function isEnum<T extends string>(value: unknown, allowed: readonly T[]): value is T {
  return typeof value === "string" && allowed.includes(value as T)
}

function isEnumArray<T extends string>(value: unknown, allowed: readonly T[]): value is T[] {
  return Array.isArray(value) && value.every((item) => isEnum(item, allowed))
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key))
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
}

function isStringRecord(value: unknown): value is Record<string, string> {
  return isRecord(value) && Object.values(value).every((item) => typeof item === "string")
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}

function isJsonObject(value: unknown): value is JsonObject {
  return isRecord(value) && Object.values(value).every(isJsonValue)
}

function isJsonValue(value: unknown): boolean {
  return value === null || typeof value === "boolean" || typeof value === "string" ||
    (typeof value === "number" && Number.isFinite(value)) ||
    (Array.isArray(value) && value.every(isJsonValue)) || isJsonObject(value)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
