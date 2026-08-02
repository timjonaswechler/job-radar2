import { invoke } from "@tauri-apps/api/core"

import type { SourceKey } from "@/lib/api/sources"
import {
  isSearchRunStatus,
  type LatestSearchRunSummary,
} from "./search-runs"
import {
  isArrayOf,
  isNonNegativeSafeInteger,
  isPositiveSafeInteger,
  isRecord,
  isString,
} from "./wire"

export type SearchRequestStatus = "draft" | "active" | "disabled"
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

export type SearchRequestValidation = {
  issues: SearchRequestValidationIssue[]
}

export type SearchRequestRecord = {
  id: number
  status: SearchRequestStatus
  includeRules: SearchRule[]
  excludeRules: SearchRule[]
  locations: string[]
  radiusKm: number | null
  sourceKeys: SourceKey[]
  validation: SearchRequestValidation
  createdAt: string
  updatedAt: string
}

/** Flat Desktop host view composed from a Catalog Record and Search Run latest summary. */
export type SearchRequest = Omit<SearchRequestRecord, "validation"> & {
  validationIssues: SearchRequestValidationIssue[]
  lastRunAt: string | null
  lastRunStatus: LatestSearchRunSummary["status"]
  lastRunError: string | null
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

export type SearchRequestErrorKind =
  | "invalid_input"
  | "not_found"
  | "busy"
  | "corrupt_stored_row"
  | "storage_unavailable"
  | "internal_invariant"

export class SearchRequestCommandError extends Error {
  constructor(
    public readonly kind: SearchRequestErrorKind,
    message: string,
    public readonly id?: number,
  ) {
    super(message)
    this.name = "SearchRequestCommandError"
  }
}

export function parseSearchRequest(value: unknown): SearchRequest | null {
  if (!isRecord(value)) return null
  if (!isPositiveSafeInteger(value.id)) return null
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

  return composeSearchRequestView(
    {
      id: value.id,
      status: value.status,
      includeRules: value.includeRules,
      excludeRules: value.excludeRules,
      locations: value.locations,
      radiusKm: value.radiusKm,
      sourceKeys: value.sourceKeys,
      validation: { issues: value.validationIssues },
      createdAt: value.createdAt,
      updatedAt: value.updatedAt,
    },
    {
      at: value.lastRunAt,
      status: value.lastRunStatus,
      error: value.lastRunError,
    },
  )
}

export function composeSearchRequestView(
  record: SearchRequestRecord,
  latest: LatestSearchRunSummary,
): SearchRequest {
  return {
    id: record.id,
    status: record.status,
    includeRules: record.includeRules,
    excludeRules: record.excludeRules,
    locations: record.locations,
    radiusKm: record.radiusKm,
    sourceKeys: record.sourceKeys,
    validationIssues: record.validation.issues,
    lastRunAt: latest.at,
    lastRunStatus: latest.status,
    lastRunError: latest.error,
    createdAt: record.createdAt,
    updatedAt: record.updatedAt,
  }
}

export async function createSearchRequest(input: CreateSearchRequestInput) {
  return requireSearchRequest(
    await invokeSearchRequest("create_search_request", { input }),
  )
}

export async function listSearchRequests() {
  const value = await invokeSearchRequest("list_search_requests")
  if (!Array.isArray(value)) throw invalidSearchRequestResponse()
  return value.map(requireSearchRequest)
}

export async function getSearchRequest(id: number) {
  return requireSearchRequest(await invokeSearchRequest("get_search_request", { id }))
}

export async function updateSearchRequest(
  id: number,
  input: UpdateSearchRequestInput,
) {
  return requireSearchRequest(
    await invokeSearchRequest("update_search_request", { id, input }),
  )
}

export async function deleteSearchRequest(id: number) {
  const value = await invokeSearchRequest("delete_search_request", { id })
  if (value !== null && value !== undefined) throw invalidSearchRequestResponse()
}

export function decodeSearchRequestError(error: unknown): Error {
  if (isRecord(error) && typeof error.message === "string") {
    const kind = error.kind as SearchRequestErrorKind
    const hasRequiredId =
      (kind === "not_found" || kind === "busy") &&
      isPositiveSafeInteger(error.id)
    const hasOptionalId =
      kind === "corrupt_stored_row" &&
      (error.id === undefined || isPositiveSafeInteger(error.id))
    const hasNoId =
      ["invalid_input", "storage_unavailable", "internal_invariant"].includes(kind) &&
      error.id === undefined
    if (hasRequiredId || hasOptionalId || hasNoId) {
      const id = isPositiveSafeInteger(error.id) ? error.id : undefined
      return new SearchRequestCommandError(kind, error.message, id)
    }
  }
  return error instanceof Error ? error : new Error(String(error))
}

async function invokeSearchRequest(
  command: string,
  args?: Record<string, unknown>,
): Promise<unknown> {
  try {
    return await invoke<unknown>(command, args)
  } catch (error) {
    throw decodeSearchRequestError(error)
  }
}

function requireSearchRequest(value: unknown) {
  const request = parseSearchRequest(value)
  if (!request) throw invalidSearchRequestResponse()
  return request
}

function invalidSearchRequestResponse() {
  return new Error("Search Request response has an invalid shape.")
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
    ].includes(String(value.code)) &&
    typeof value.path === "string" &&
    typeof value.message === "string"
  )
}

function isSearchRequestStatus(value: unknown): value is SearchRequestStatus {
  return value === "draft" || value === "active" || value === "disabled"
}
