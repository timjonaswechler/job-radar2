import { describe, expect, it } from "vitest"
import {
  PostingTransportError,
  decodeJobPosting,
  decodeJobPostingDetail,
  decodeJobPostingQueueCounts,
  decodePostingTransportError,
} from "./job-postings"
const occurrence = {
  id: 11, sourceKey: "acme", sourceNameSnapshot: "Acme Careers",
  identity: { kind: "provider_posting_id", value: "job-42" },
  providerUrl: "https://example.test/jobs/42", postingMeta: { requisitionId: "req-42" },
  firstSeenAt: "2026-07-05T10:00:00.000Z", lastSeenAt: "2026-07-05T11:00:00.000Z",
}
const posting = {
  id: 7, title: "Engineer", company: "Acme", locations: ["Berlin"], descriptionText: null,
  readState: "unread", interestState: "undecided", preparationState: "not_started",
  applicationState: "not_applied", primaryQueue: "inbox",
  firstSeenAt: occurrence.firstSeenAt, lastSeenAt: occurrence.lastSeenAt,
  createdAt: occurrence.firstSeenAt, updatedAt: occurrence.firstSeenAt,
  primaryOccurrence: occurrence, occurrences: [occurrence],
}
const diagnostic = {
  category: "runtime", code: "detail_failed", message: "failed", severity: "error",
  path: "/detail", strategyKey: "html", details: { attempts: 1 },
}
describe("Job Posting transport", () => {
  it("decodes Posting and Source-local occurrence wire values", () => {
    expect(decodeJobPosting(posting)).toEqual(posting)
    expect(decodeJobPosting({
      ...posting,
      primaryOccurrence: { ...occurrence, identity: { kind: "normalized_url", value: occurrence.providerUrl } },
    }).primaryOccurrence.identity.kind).toBe("normalized_url")
  })
  it.each([
    ["unsafe Posting ID", { ...posting, id: Number.MAX_SAFE_INTEGER + 1 }],
    ["zero Posting ID", { ...posting, id: 0 }],
    ["unsafe occurrence ID", { ...posting, occurrences: [{ ...occurrence, id: 1.5 }] }],
    ["identity value", { ...posting, primaryOccurrence: { ...occurrence, identity: { kind: "provider_posting_id" } } }],
    ["provider URL", { ...posting, primaryOccurrence: { ...occurrence, providerUrl: 42 } }],
    ["postingMeta", { ...posting, primaryOccurrence: { ...occurrence, postingMeta: { id: 42 } } }],
    ["locations", { ...posting, locations: ["Berlin", 42] }],
    ["read state", { ...posting, readState: "new" }],
    ["interest state", { ...posting, interestState: "maybe" }],
    ["preparation state", { ...posting, preparationState: "done" }],
    ["application state", { ...posting, applicationState: "applied" }],
    ["primary queue", { ...posting, primaryQueue: "all" }],
  ])("rejects malformed %s", (_label, value) => {
    expect(() => decodeJobPosting(value)).toThrow(PostingTransportError)
  })
  it("types a corrupt occurrence identity without discarding recovered Detail", () => {
    const invalid = { ...occurrence, identity: { kind: "broken", value: "x" } }
    const decoded = decodeJobPostingDetail({
      ...posting, primaryOccurrence: invalid, occurrences: [invalid, occurrence],
      descriptionState: { status: "loaded", text: "Recovered", diagnostics: [diagnostic] },
    })
    expect(decoded.primaryOccurrence.identity).toEqual({ kind: "invalid", rawKind: "broken", value: "x" })
    expect(decoded.descriptionState).toMatchObject({ status: "loaded", text: "Recovered" })
  })
  it("accepts every workflow state and primary queue", () => {
    for (const [field, values] of Object.entries({
      readState: ["unread", "read"], interestState: ["undecided", "interested", "dismissed"],
      preparationState: ["not_started", "in_progress", "ready"],
      applicationState: ["not_applied", "submitted", "in_process", "rejected_by_company", "withdrawn_by_me", "accepted"],
      primaryQueue: ["inbox", "interested", "preparation", "applied", "archive"],
    })) for (const value of values) expect(decodeJobPosting({ ...posting, [field]: value })).toMatchObject({ [field]: value })
  })
  it("decodes bounded Counts", () => {
    const counts = { inbox: 2, interested: 1, preparation: 3, applied: 4, archive: 5, all: 15, newInbox: 1, reviewInbox: 1 }
    expect(decodeJobPostingQueueCounts(counts)).toEqual(counts)
    expect(() => decodeJobPostingQueueCounts({ ...counts, inbox: -1 })).toThrow(PostingTransportError)
    expect(() => decodeJobPostingQueueCounts({ ...counts, all: 1.5 })).toThrow(PostingTransportError)
  })
  it.each([
    { status: "loaded", text: "Description", diagnostics: [diagnostic] },
    { status: "unsupported", message: "unsupported", diagnostics: [diagnostic] },
    { status: "failed", message: "failed", diagnostics: [diagnostic] },
  ])("decodes $status description outcomes and Diagnostics", (descriptionState) => {
    expect(decodeJobPostingDetail({ ...posting, descriptionState }).descriptionState).toEqual(descriptionState)
  })
  it("rejects malformed description outcomes and Diagnostics", () => {
    expect(() => decodeJobPostingDetail({ ...posting, descriptionState: { status: "loaded", diagnostics: [] } })).toThrow(PostingTransportError)
    expect(() => decodeJobPostingDetail({ ...posting, descriptionState: { status: "failed", message: "x", diagnostics: [{ ...diagnostic, severity: "fatal" }] } })).toThrow(PostingTransportError)
  })
  it("preserves structured command errors and bounds arbitrary rejections", () => {
    const errors = [
      { kind: "not_found", postingId: 7, message: "missing" }, { kind: "invalid_change", message: "empty" },
      { kind: "corrupt", postingId: 7, message: "corrupt" }, { kind: "storage", message: "offline" },
      { kind: "before_read", message: "not opened" }, { kind: "after_read", postingId: 7, message: "failed" },
    ] as const
    for (const error of errors) expect(decodePostingTransportError(error)).toMatchObject(error)
    expect(decodePostingTransportError("offline")).toMatchObject({ kind: "transport", message: "offline" })
  })
})
