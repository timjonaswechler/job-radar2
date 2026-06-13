import { invoke } from "@tauri-apps/api/core"

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
  sourceIds: number[]
  validationError: string | null
  createdAt: string
  updatedAt: string
}

export type CreateSearchRequestInput = {
  status: SearchRequestStatus
  includeRules: SearchRule[]
  excludeRules: SearchRule[]
  locations: string[]
  radiusKm: number | null
  sourceIds: number[]
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
