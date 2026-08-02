import { invoke } from "@tauri-apps/api/core"

import type { StructuredDiagnostic } from "@/lib/api/sources"
import { decodeBackgroundTaskSnapshot, type BackgroundTaskSnapshot } from "./background-tasks"
import {
  isArrayOf,
  isNonNegativeSafeInteger,
  isPositiveSafeInteger,
  isRecord,
  isString,
  isStructuredDiagnostic,
} from "./wire"

export type SearchRunStatus =
  | "completed"
  | "completed_with_errors"
  | "failed"
  | "cancelled"

export type LatestSearchRunSummary = {
  at: string | null
  status: SearchRunStatus | null
  error: string | null
}

export type SourceRunStatus = "completed" | "failed" | "cancelled" | "skipped"

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

export function parseSearchRunResult(value: unknown): SearchRunResult | null {
  if (!isRecord(value)) return null
  if (!isPositiveSafeInteger(value.searchRequestId)) return null
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

export async function runSearchRequest(id: number): Promise<BackgroundTaskSnapshot> {
  return decodeBackgroundTaskSnapshot(
    await invoke<unknown>("run_search_request", { id }),
  )
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
    isArrayOf(summary.samples, isStructuredDiagnostic) &&
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

export function isSearchRunStatus(value: unknown): value is SearchRunStatus {
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
