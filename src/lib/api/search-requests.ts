import { invoke } from "@tauri-apps/api/core"

import type { SourceKey, StructuredDiagnostic } from "@/lib/api/sources"

export type SearchRequestStatus = "draft" | "active" | "disabled" | "invalid"

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

export type SearchRequest = {
  id: number
  status: SearchRequestStatus
  includeRules: SearchRule[]
  excludeRules: SearchRule[]
  locations: string[]
  radiusKm: number | null
  sourceKeys: SourceKey[]
  validationError: string | null
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

export type SourceRunResult = {
  sourceKey: string
  sourceName: string
  status: SourceRunStatus
  candidateCount: number
  matchedCount: number
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

export function parseSearchRunResult(value: unknown): SearchRunResult | null {
  if (!isRecord(value)) return null
  if (typeof value.searchRequestId !== "number") return null
  if (!isSearchRunStatus(value.status)) return null
  if (typeof value.generatedAt !== "string") return null
  if (!Array.isArray(value.diagnostics)) return null
  if (!Array.isArray(value.sourceRuns)) return null

  const sourceRuns = value.sourceRuns.filter(isSourceRunResult)
  if (sourceRuns.length !== value.sourceRuns.length) return null

  return {
    searchRequestId: value.searchRequestId,
    status: value.status,
    generatedAt: value.generatedAt,
    diagnostics: value.diagnostics as StructuredDiagnostic[],
    sourceRuns,
    postings: Array.isArray(value.postings)
      ? (value.postings as NormalizedPosting[])
      : [],
  }
}

function isSourceRunResult(value: unknown): value is SourceRunResult {
  return (
    isRecord(value) &&
    typeof value.sourceKey === "string" &&
    typeof value.sourceName === "string" &&
    isSourceRunStatus(value.status) &&
    typeof value.candidateCount === "number" &&
    typeof value.matchedCount === "number" &&
    Array.isArray(value.diagnostics) &&
    (typeof value.error === "string" || value.error === null)
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
  return typeof value === "object" && value !== null
}

export function createSearchRequest(input: CreateSearchRequestInput) {
  return invoke<SearchRequest>("create_search_request", { input })
}

export function listSearchRequests() {
  return invoke<SearchRequest[]>("list_search_requests")
}

export function getSearchRequest(id: number) {
  return invoke<SearchRequest>("get_search_request", { id })
}

export function updateSearchRequest(
  id: number,
  input: UpdateSearchRequestInput,
) {
  return invoke<SearchRequest>("update_search_request", { id, input })
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
