// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest"
import assert from "node:assert/strict"
import { render, screen } from "@testing-library/react"
import { createElement, Fragment } from "react"
import { expect, test } from "vitest"
import { PostingsListPanel } from "@/features/postings/list/postings-list-panel"
import { PostingPreviewPanel } from "@/features/postings/preview/posting-preview-panel"
import { getPrimaryQueueLabel, getQueueDefinition, isPostingQueuePathActive } from "@/features/postings/queues/posting-queues"
import { createPostingItemViewModel, createPostingPreparationProgressViewModel } from "@/features/postings/view-model/posting-item-view-model"
import type { JobPosting, JobPostingDetail } from "@/lib/api/job-postings"
test("postings presentation consumes transported workflow projections", () => {
  assert.equal(isPostingQueuePathActive("/postings", "inbox"), true)
  assert.equal(isPostingQueuePathActive("/postings/inbox", "inbox"), true)
  assert.equal(isPostingQueuePathActive("/settings", "inbox"), false)
  assert.equal(isPostingQueuePathActive("/postings-extra", "inbox"), false)
  const posting = createPosting({
    interestState: "dismissed",
    preparationState: "ready",
    primaryQueue: "interested",
  })
  assert.equal(
    getPrimaryQueueLabel(posting),
    "Interessant",
    "labels use transported primaryQueue rather than classifying workflow axes",
  )
  const item = createPostingItemViewModel(posting)
  assert.equal(item.preview.workflow.queueLabel, "Interessant")
  assert.equal(item.preview.workflow.primarySourceLabel, "Acme Careers")
  assert.equal(item.row.preparationProgress, null)
  const detail: JobPostingDetail = {
    ...posting,
    descriptionState: { status: "loaded", text: "Visible description", diagnostics: [] },
  }
  render(createElement(Fragment, null,
    createElement(PostingsListPanel, {
      activeQueue: getQueueDefinition("interested"), error: null, loading: false,
      postings: [item.row], selectedPostingId: posting.id,
      onRetry: async () => {}, onSelectPosting: () => {},
    }),
    createElement(PostingPreviewPanel, {
      detailState: { status: "loaded", postingId: posting.id, detail },
      loading: false, posting: item.preview,
    }),
  ))
  expect(screen.getAllByText("Interessant").length).toBeGreaterThan(0)
  expect(screen.getAllByText("Product Engineer").length).toBeGreaterThan(0)
  expect(screen.getAllByText("Visible description").length).toBeGreaterThan(0)
  const progress = createPostingPreparationProgressViewModel({
    applicationState: "not_applied",
    tasks: [
      { task: "documents_ready", status: "completed" },
      { task: "company_research", status: "in_progress" },
      { task: "posting_data_ready", status: "completed" },
      { task: "cover_letter", status: "not_applicable" },
      { task: "strategy_notes", status: "not_started" },
    ],
  })
  assert.ok(progress)
  assert.deepEqual(progress.steps.map(({ task, status }) => [task, status]), [
    ["posting_data_ready", "completed"],
    ["company_research", "in_progress"],
    ["strategy_notes", "not_started"],
    ["cover_letter", "not_applicable"],
    ["documents_ready", "completed"],
  ])
  assert.equal(progress.leadLabel, "Als Nächstes: Firmenrecherche")
})
function createPosting(overrides: Partial<JobPosting> = {}): JobPosting {
  const occurrence = {
    id: 11, sourceKey: "acme", sourceNameSnapshot: "Acme Careers",
    identity: { kind: "provider_posting_id" as const, value: "job-1" },
    providerUrl: "https://example.test/jobs/1", postingMeta: {},
    firstSeenAt: "2026-07-05T10:00:00.000Z", lastSeenAt: "2026-07-05T11:00:00.000Z",
  }
  return {
    id: 1, title: "Product Engineer", company: "Acme GmbH", locations: ["Berlin"],
    descriptionText: null, readState: "unread", interestState: "undecided",
    preparationState: "not_started", applicationState: "not_applied", primaryQueue: "inbox",
    firstSeenAt: occurrence.firstSeenAt, lastSeenAt: occurrence.lastSeenAt,
    createdAt: occurrence.firstSeenAt, updatedAt: occurrence.firstSeenAt,
    primaryOccurrence: occurrence, occurrences: [occurrence], ...overrides,
  }
}
