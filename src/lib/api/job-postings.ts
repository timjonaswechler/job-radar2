import { invoke } from "@tauri-apps/api/core"

import type { SourceKey, StructuredDiagnostic } from "@/lib/api/sources"
import {
  isArrayOf,
  isNonNegativeSafeInteger,
  isPositiveSafeInteger,
  isRecord,
  isString,
  isStructuredDiagnostic,
} from "@/lib/api/wire"

const primaryQueueIds = ["inbox", "interested", "preparation", "applied", "archive"] as const
const readStates = ["unread", "read"] as const
const interestStates = ["undecided", "interested", "dismissed"] as const
const preparationStates = ["not_started", "in_progress", "ready"] as const
const applicationStates = [
  "not_applied", "submitted", "in_process", "rejected_by_company", "withdrawn_by_me", "accepted",
] as const
const commandErrorKinds = [
  "not_found", "invalid_change", "corrupt", "storage", "before_read", "after_read",
] as const

export type JobPostingPrimaryQueueId = (typeof primaryQueueIds)[number]
export type JobPostingQueueId = JobPostingPrimaryQueueId | "all"
export type JobPostingReadState = (typeof readStates)[number]
export type JobPostingInterestState = (typeof interestStates)[number]
export type JobPostingPreparationState = (typeof preparationStates)[number]
export type JobPostingApplicationState = (typeof applicationStates)[number]

export type PostingOccurrenceIdentity =
  | { kind: "provider_posting_id" | "normalized_url"; value: string }
  | { kind: "invalid"; rawKind: string; value: string }
export type PostingOccurrence = {
  id: number
  sourceKey: SourceKey
  sourceNameSnapshot: string
  identity: PostingOccurrenceIdentity
  providerUrl: string
  postingMeta: Record<string, string>
  firstSeenAt: string
  lastSeenAt: string
}
export type JobPosting = {
  id: number
  title: string
  company: string
  locations: string[]
  descriptionText: string | null
  readState: JobPostingReadState
  interestState: JobPostingInterestState
  preparationState: JobPostingPreparationState
  applicationState: JobPostingApplicationState
  primaryQueue: JobPostingPrimaryQueueId
  firstSeenAt: string
  lastSeenAt: string
  createdAt: string
  updatedAt: string
  primaryOccurrence: PostingOccurrence
  occurrences: PostingOccurrence[]
}
export type PostingDescriptionState =
  | { status: "loaded"; text: string; diagnostics: StructuredDiagnostic[] }
  | { status: "unsupported"; message: string; diagnostics: StructuredDiagnostic[] }
  | { status: "failed"; message: string; diagnostics: StructuredDiagnostic[] }
export type JobPostingDetail = JobPosting & { descriptionState: PostingDescriptionState }
export type JobPostingQueueCounts = Record<JobPostingQueueId, number> & {
  newInbox: number
  reviewInbox: number
}

export type PostingTransportErrorKind =
  | (typeof commandErrorKinds)[number]
  | "invalid_response"
  | "transport"
export class PostingTransportError extends Error {
  readonly kind: PostingTransportErrorKind
  readonly postingId?: number

  constructor(kind: PostingTransportErrorKind, message: string, postingId?: number) {
    super(message)
    this.name = "PostingTransportError"
    this.kind = kind
    this.postingId = postingId
  }
}

export function listJobPostingsForQueue(queueId: JobPostingQueueId): Promise<JobPosting[]> {
  return call("list_job_postings_for_queue", { queueId }, decodeJobPostingList)
}
export function getPostingDetail(postingId: number): Promise<JobPostingDetail> {
  if (!isPositiveSafeInteger(postingId)) invalid("Posting ID")
  return call("get_job_posting", { postingId }, decodeJobPostingDetail)
}
export function getJobPostingQueueCounts(): Promise<JobPostingQueueCounts> {
  return call("get_job_posting_queue_counts", undefined, decodeJobPostingQueueCounts)
}

async function call<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  decode: (value: unknown) => T,
): Promise<T> {
  try {
    return decode(await invoke<unknown>(command, args))
  } catch (error) {
    throw decodePostingTransportError(error)
  }
}

export function decodeJobPostingList(value: unknown): JobPosting[] {
  if (!Array.isArray(value)) invalid("Posting list")
  return value.map(decodeJobPosting)
}

export function decodeJobPosting(value: unknown): JobPosting {
  if (
    !isRecord(value) || !isPositiveSafeInteger(value.id) ||
    !isString(value.title) || !isString(value.company) ||
    !isArrayOf(value.locations, isString) ||
    (value.descriptionText !== null && !isString(value.descriptionText)) ||
    !includes(readStates, value.readState) || !includes(interestStates, value.interestState) ||
    !includes(preparationStates, value.preparationState) ||
    !includes(applicationStates, value.applicationState) ||
    !includes(primaryQueueIds, value.primaryQueue) ||
    !isString(value.firstSeenAt) || !isString(value.lastSeenAt) ||
    !isString(value.createdAt) || !isString(value.updatedAt) ||
    !Array.isArray(value.occurrences)
  ) invalid("Posting")

  const primaryOccurrence = decodePostingOccurrence(value.primaryOccurrence)
  const occurrences = value.occurrences.map(decodePostingOccurrence)
  if (!occurrences.some(({ id }) => id === primaryOccurrence.id)) {
    invalid("Posting primary occurrence")
  }
  return { ...value, primaryOccurrence, occurrences } as JobPosting
}

export function decodePostingOccurrence(value: unknown): PostingOccurrence {
  if (
    !isRecord(value) || !isPositiveSafeInteger(value.id) ||
    !isString(value.sourceKey) || !isString(value.sourceNameSnapshot) ||
    !isRecord(value.identity) || !isString(value.identity.kind) ||
    !isString(value.identity.value) || !isString(value.providerUrl) ||
    !isStringRecord(value.postingMeta) ||
    !isString(value.firstSeenAt) || !isString(value.lastSeenAt)
  ) invalid("Posting occurrence")
  const identity = includes(
    ["provider_posting_id", "normalized_url"] as const,
    value.identity.kind,
  )
    ? value.identity as PostingOccurrenceIdentity
    : { kind: "invalid" as const, rawKind: value.identity.kind, value: value.identity.value }
  return { ...value, identity } as PostingOccurrence
}

export function decodeJobPostingDetail(value: unknown): JobPostingDetail {
  const posting = decodeJobPosting(value)
  if (!isRecord(value)) invalid("Posting Detail")
  return { ...posting, descriptionState: decodeDescriptionState(value.descriptionState) }
}

export function decodeJobPostingQueueCounts(value: unknown): JobPostingQueueCounts {
  if (!isRecord(value)) invalid("Posting queue Counts")
  const keys = [...primaryQueueIds, "all", "newInbox", "reviewInbox"] as const
  if (keys.some((key) => !isNonNegativeSafeInteger(value[key]))) {
    invalid("Posting queue Counts")
  }
  return value as JobPostingQueueCounts
}

function decodeDescriptionState(value: unknown): PostingDescriptionState {
  if (!isRecord(value) || !Array.isArray(value.diagnostics)) {
    invalid("Posting description outcome")
  }
  const diagnostics = value.diagnostics
  if (!diagnostics.every(isStructuredDiagnostic)) invalid("Posting description Diagnostics")
  if (value.status === "loaded" && isString(value.text)) {
    return { status: "loaded", text: value.text, diagnostics }
  }
  if ((value.status === "unsupported" || value.status === "failed") && isString(value.message)) {
    return { status: value.status, message: value.message, diagnostics }
  }
  invalid("Posting description outcome")
}

export function decodePostingTransportError(value: unknown): PostingTransportError {
  if (value instanceof PostingTransportError) return value
  if (isRecord(value) && includes(commandErrorKinds, value.kind) && isString(value.message)) {
    const needsId = ["not_found", "corrupt", "after_read"].includes(value.kind)
    if (!needsId) return new PostingTransportError(value.kind, value.message)
    if (isPositiveSafeInteger(value.postingId)) {
      return new PostingTransportError(value.kind, value.message, value.postingId)
    }
  }
  if (value instanceof Error) return new PostingTransportError("transport", value.message)
  return new PostingTransportError(
    "transport",
    typeof value === "string" ? value : "Job Posting transport failed",
  )
}

function invalid(label: string): never {
  throw new PostingTransportError("invalid_response", `invalid ${label} transport`)
}
function includes<T extends string>(values: readonly T[], value: unknown): value is T {
  return typeof value === "string" && values.includes(value as T)
}
function isStringRecord(value: unknown): value is Record<string, string> {
  return isRecord(value) && Object.values(value).every(isString)
}
