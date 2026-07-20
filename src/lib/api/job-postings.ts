import { invoke } from "@tauri-apps/api/core"

import type { SourceKey, StructuredDiagnostic } from "@/lib/api/sources"

export type JobPostingQueueId =
  | "inbox"
  | "interested"
  | "preparation"
  | "applied"
  | "archive"
  | "all"

export type JobPostingReadState = "unread" | "read"

export type JobPostingInterestState =
  | "undecided"
  | "interested"
  | "dismissed"

export type JobPostingPreparationState =
  | "not_started"
  | "in_progress"
  | "ready"

export type JobPostingApplicationState =
  | "not_applied"
  | "submitted"
  | "in_process"
  | "rejected_by_company"
  | "withdrawn_by_me"
  | "accepted"

export type JobPostingSource = {
  id: number
  sourceKey: SourceKey
  sourceNameSnapshot: string
  url: string
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
  firstSeenAt: string
  lastSeenAt: string
  createdAt: string
  updatedAt: string
  primarySource: JobPostingSource | null
  sources: JobPostingSource[]
}

export type PostingDescriptionState =
  | { status: "loaded"; text: string; diagnostics: StructuredDiagnostic[] }
  | { status: "unsupported"; message: string; diagnostics: StructuredDiagnostic[] }
  | { status: "failed"; message: string; diagnostics: StructuredDiagnostic[] }

export type JobPostingDetail = JobPosting & {
  descriptionState: PostingDescriptionState
}

export type JobPostingQueueCounts = Record<JobPostingQueueId, number> & {
  newInbox: number
  reviewInbox: number
}

export type UpdateJobPostingStateInput = {
  readState?: JobPostingReadState
  interestState?: JobPostingInterestState
  preparationState?: JobPostingPreparationState
  applicationState?: JobPostingApplicationState
}

export function listJobPostings() {
  return invoke<JobPosting[]>("list_job_postings")
}

export function listJobPostingsForQueue(queueId: JobPostingQueueId) {
  return invoke<JobPosting[]>("list_job_postings_for_queue", { queueId })
}

export function getPostingDetail(postingId: number) {
  return invoke<JobPostingDetail>("get_job_posting", { postingId })
}

export function getJobPostingQueueCounts() {
  return invoke<JobPostingQueueCounts>("get_job_posting_queue_counts")
}

export function updateJobPostingState(
  id: number,
  input: UpdateJobPostingStateInput,
) {
  return invoke<JobPosting>("update_job_posting_state", { id, input })
}
