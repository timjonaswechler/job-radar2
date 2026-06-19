import { invoke } from "@tauri-apps/api/core"

import type { SourceKey } from "@/lib/api/sources"

export type SearchRequestStatus = "draft" | "active" | "disabled" | "invalid"

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
  createdAt: string
  updatedAt: string
}

export type SearchRunStatus =
  | "completed"
  | "completed_with_errors"
  | "failed"
  | "cancelled"

export type SourceRunStatus = "completed" | "failed" | "cancelled"

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
  error: string | null
}

export type SearchRunResult = {
  searchRequestId: number
  status: SearchRunStatus
  generatedAt: string
  sourceRuns: SourceRunResult[]
  postings: NormalizedPosting[]
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
  return invoke<SearchRunResult>("run_search_request", { id })
}
