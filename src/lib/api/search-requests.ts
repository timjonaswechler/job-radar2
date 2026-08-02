import { invoke } from "@tauri-apps/api/core"

import type { SourceKey, StructuredDiagnostic } from "@/lib/api/sources"

export type SearchRequestStatus = "draft" | "active" | "disabled"

export type SearchRunStatus =
  | "completed"
  | "completed_with_errors"
  | "failed"
  | "cancelled"

export type SearchRuleTarget = "title"

export type SearchRuleKind = "text" | "regex"

export type SearchRule = {
  target: SearchRuleTarget
  kind: SearchRuleKind
  value: string
}

export type SearchRequestValidationIssueCode =
  | "invalid_regex"
  | "include_rule_required"
  | "source_key_required"
  | "duplicate_source_key"
  | "issues_truncated"

export type SearchRequestValidationIssue = {
  code: SearchRequestValidationIssueCode
  path: string
  message: string
}

export type SearchRequest = {
  id: number
  status: SearchRequestStatus
  includeRules: SearchRule[]
  excludeRules: SearchRule[]
  locations: string[]
  radiusKm: number | null
  sourceKeys: SourceKey[]
  validationIssues: SearchRequestValidationIssue[]
  lastRunAt: string | null
  lastRunStatus: SearchRunStatus | null
  lastRunError: string | null
  createdAt: string
  updatedAt: string
}

export type SourceRunStatus = "completed" | "failed" | "cancelled" | "skipped"

export type BackgroundTaskState =
  | "queued"
  | "running"
  | "cancelling"
  | "succeeded"
  | "failed"
  | "cancelled"

export type BackgroundTaskKind = "search_run" | { other: string }

export type BackgroundTaskProgress = {
  message: string
  current: number | null
  total: number | null
}

export type PostingSource = {
  sourceKey: string
  sourceName: string
  url: string
}

export type NormalizedPosting = {
  title: string
  company: string
  url: string
  locations: string[]
  sources: PostingSource[]
}

export type ResolutionCounts = {
  discovered: number
  processed: number
  finalized: number
  rejected: number
  unresolved: number
  failed: number
  budgetSkipped: number
}

export type ResolutionLimitDimension =
  | "discovery_batches"
  | "discovered_items"
  | "detail_candidates"
  | "strategy_attempts"
  | "requests"
  | "produced_items"
  | "duration"
  | "pages"
  | "browser_actions"
  | "fan_out"
  | "response_bytes"
  | "browser_rendered_bytes"

export type SourceResolutionSummary = {
  completion:
    | { type: "complete" }
    | { type: "partial"; limitReached: ResolutionLimitDimension }
  counts: ResolutionCounts
  remaining: number | null
  usage: Record<string, number>
  candidateDiagnostics: {
    countsByCode: Record<string, number>
    samples: StructuredDiagnostic[]
    sampleLimit: number
    candidateDiagnosticsOmitted: number
  }
}

export type SourceRunResult = {
  sourceKey: string
  sourceName: string
  status: SourceRunStatus
  resolution: SourceResolutionSummary | null
  diagnostics: StructuredDiagnostic[]
  error: string | null
}

export type SearchRunResult = {
  searchRequestId: number
  status: SearchRunStatus
  generatedAt: string
  diagnostics: StructuredDiagnostic[]
  sourceRuns: SourceRunResult[]
  postings: NormalizedPosting[]
}

export type BackgroundTaskSnapshot = {
  taskId: string
  kind: BackgroundTaskKind
  state: BackgroundTaskState
  progress: BackgroundTaskProgress | null
  result: unknown | null
  error: string | null
  diagnostics: StructuredDiagnostic[]
}

export type CreateSearchRequestInput = {
  status: SearchRequestStatus
  includeRules: SearchRule[]
  excludeRules: SearchRule[]
  locations: string[]
  radiusKm: number | null
  sourceKeys: SourceKey[]
}

export type UpdateSearchRequestInput = CreateSearchRequestInput

export function parseSearchRequest(value: unknown): SearchRequest | null {
  if (!isRecord(value)) return null
  if (!isNonNegativeSafeInteger(value.id)) return null
  if (!isSearchRequestStatus(value.status)) return null
  if (!isArrayOf(value.includeRules, isSearchRule)) return null
  if (!isArrayOf(value.excludeRules, isSearchRule)) return null
  if (!isArrayOf(value.locations, isString)) return null
  if (!(value.radiusKm === null || isNonNegativeSafeInteger(value.radiusKm))) return null
  if (!isArrayOf(value.sourceKeys, isString)) return null
  if (!isArrayOf(value.validationIssues, isSearchRequestValidationIssue)) return null
  if (value.validationIssues.length > 64) return null
  if (!(value.lastRunAt === null || typeof value.lastRunAt === "string")) return null
  if (!(value.lastRunStatus === null || isSearchRunStatus(value.lastRunStatus))) return null
  if (!(value.lastRunError === null || typeof value.lastRunError === "string")) return null
  if (typeof value.createdAt !== "string" || typeof value.updatedAt !== "string") return null

  return {
    id: value.id,
    status: value.status,
    includeRules: value.includeRules,
    excludeRules: value.excludeRules,
    locations: value.locations,
    radiusKm: value.radiusKm,
    sourceKeys: value.sourceKeys,
    validationIssues: value.validationIssues,
    lastRunAt: value.lastRunAt,
    lastRunStatus: value.lastRunStatus,
    lastRunError: value.lastRunError,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  }
}

function isSearchRule(value: unknown): value is SearchRule {
  return (
    isRecord(value) &&
    value.target === "title" &&
    (value.kind === "text" || value.kind === "regex") &&
    typeof value.value === "string"
  )
}

function isSearchRequestValidationIssue(
  value: unknown,
): value is SearchRequestValidationIssue {
  return (
    isRecord(value) &&
    [
      "invalid_regex",
      "include_rule_required",
      "source_key_required",
      "duplicate_source_key",
      "issues_truncated",
    ].includes(value.code as SearchRequestValidationIssueCode) &&
    typeof value.path === "string" &&
    typeof value.message === "string"
  )
}

function isSearchRequestStatus(value: unknown): value is SearchRequestStatus {
  return value === "draft" || value === "active" || value === "disabled"
}

export function parseSearchRunResult(value: unknown): SearchRunResult | null {
  if (!isRecord(value)) return null
  if (!isNonNegativeSafeInteger(value.searchRequestId)) return null
  if (!isSearchRunStatus(value.status)) return null
  if (typeof value.generatedAt !== "string") return null
  if (!isArrayOf(value.diagnostics, isStructuredDiagnostic)) return null
  if (!isArrayOf(value.sourceRuns, isSourceRunResult)) return null
  if (!isArrayOf(value.postings, isNormalizedPosting)) return null

  return {
    searchRequestId: value.searchRequestId,
    status: value.status,
    generatedAt: value.generatedAt,
    diagnostics: value.diagnostics,
    sourceRuns: value.sourceRuns,
    postings: value.postings,
  }
}

function isNormalizedPosting(value: unknown): value is NormalizedPosting {
  return (
    isRecord(value) &&
    typeof value.title === "string" &&
    typeof value.company === "string" &&
    typeof value.url === "string" &&
    isArrayOf(value.locations, isString) &&
    isArrayOf(value.sources, isPostingSource)
  )
}

function isPostingSource(value: unknown): value is PostingSource {
  return (
    isRecord(value) &&
    typeof value.sourceKey === "string" &&
    typeof value.sourceName === "string" &&
    typeof value.url === "string"
  )
}

function isSourceRunResult(value: unknown): value is SourceRunResult {
  return (
    isRecord(value) &&
    typeof value.sourceKey === "string" &&
    typeof value.sourceName === "string" &&
    isSourceRunStatus(value.status) &&
    (value.resolution === null || isSourceResolutionSummary(value.resolution)) &&
    isArrayOf(value.diagnostics, isStructuredDiagnostic) &&
    (typeof value.error === "string" || value.error === null)
  )
}

function isSourceResolutionSummary(value: unknown): value is SourceResolutionSummary {
  if (!isRecord(value) || !isRecord(value.counts)) return false
  const countFields = [
    "discovered", "processed", "finalized", "rejected", "unresolved", "failed", "budgetSkipped",
  ]
  const usageFields = [
    "strategyAttempts", "requests", "producedItems", "durationMs", "pages",
    "browserActions", "fanOut", "responseBytes", "browserRenderedBytes",
  ]
  const counts = value.counts
  if (!countFields.every((key) => isNonNegativeSafeInteger(counts[key]))) return false
  if (!isResolutionCompletion(value.completion)) return false
  if (!(value.remaining === null || isNonNegativeSafeInteger(value.remaining))) return false
  if (!isRecord(value.usage)) return false
  const usage = value.usage
  if (!usageFields.every((key) => isNonNegativeSafeInteger(usage[key]))) return false
  if (!Object.values(usage).every(isNonNegativeSafeInteger)) return false
  if (!isRecord(value.candidateDiagnostics)) return false
  const summary = value.candidateDiagnostics
  return (
    isRecord(summary.countsByCode) &&
    Object.values(summary.countsByCode).every(isNonNegativeSafeInteger) &&
    Array.isArray(summary.samples) &&
    summary.samples.every(isStructuredDiagnostic) &&
    isNonNegativeSafeInteger(summary.sampleLimit) &&
    isNonNegativeSafeInteger(summary.candidateDiagnosticsOmitted)
  )
}

function isResolutionCompletion(value: unknown): boolean {
  if (!isRecord(value)) return false
  if (value.type === "complete") return !("limitReached" in value)
  return value.type === "partial" && isResolutionLimitDimension(value.limitReached)
}

function isResolutionLimitDimension(value: unknown): value is ResolutionLimitDimension {
  return [
    "discovery_batches", "discovered_items", "detail_candidates", "strategy_attempts",
    "requests", "produced_items", "duration", "pages", "browser_actions", "fan_out",
    "response_bytes", "browser_rendered_bytes",
  ].includes(value as ResolutionLimitDimension)
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}

function isArrayOf<T>(
  value: unknown,
  predicate: (entry: unknown) => entry is T,
): value is T[] {
  return Array.isArray(value) && value.every(predicate)
}

function isString(value: unknown): value is string {
  return typeof value === "string"
}

function isStructuredDiagnostic(value: unknown): value is StructuredDiagnostic {
  return (
    isRecord(value) &&
    ["schema", "registry", "compiler", "runtime", "detection", "source_validation"].includes(value.category as string) &&
    typeof value.code === "string" &&
    typeof value.message === "string" &&
    ["info", "warning", "error"].includes(value.severity as string) &&
    typeof value.path === "string" &&
    (value.strategyKey === undefined || typeof value.strategyKey === "string")
  )
}

function isSearchRunStatus(value: unknown): value is SearchRunStatus {
  return (
    value === "completed" ||
    value === "completed_with_errors" ||
    value === "failed" ||
    value === "cancelled"
  )
}

function isSourceRunStatus(value: unknown): value is SourceRunStatus {
  return (
    value === "completed" ||
    value === "failed" ||
    value === "cancelled" ||
    value === "skipped"
  )
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

export async function createSearchRequest(input: CreateSearchRequestInput) {
  return requireSearchRequest(
    await invoke<unknown>("create_search_request", { input }),
  )
}

export async function listSearchRequests() {
  const value = await invoke<unknown>("list_search_requests")
  if (!Array.isArray(value)) throw invalidSearchRequestResponse()
  return value.map(requireSearchRequest)
}

export async function getSearchRequest(id: number) {
  return requireSearchRequest(await invoke<unknown>("get_search_request", { id }))
}

export async function updateSearchRequest(
  id: number,
  input: UpdateSearchRequestInput,
) {
  return requireSearchRequest(
    await invoke<unknown>("update_search_request", { id, input }),
  )
}

function requireSearchRequest(value: unknown) {
  const request = parseSearchRequest(value)
  if (!request) throw invalidSearchRequestResponse()
  return request
}

function invalidSearchRequestResponse() {
  return new Error("Search Request response has an invalid shape.")
}

export function deleteSearchRequest(id: number) {
  return invoke<void>("delete_search_request", { id })
}

export function runSearchRequest(id: number) {
  return invoke<BackgroundTaskSnapshot>("run_search_request", { id })
}

export function getBackgroundTask(taskId: string) {
  return invoke<BackgroundTaskSnapshot>("get_background_task", { taskId })
}

export function cancelBackgroundTask(taskId: string) {
  return invoke<BackgroundTaskSnapshot>("cancel_background_task", { taskId })
}
