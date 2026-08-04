// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest"
import { act, cleanup, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { SidebarProvider } from "@/components/ui/sidebar"
import { PostingsSidebar } from "@/features/postings/queues/postings-sidebar"
import {
  PostingsWorkspaceProvider,
  usePostingsList,
} from "@/features/postings/workspace/postings-workspace-provider"
import {
  PostingTransportError,
  type JobPosting,
  type JobPostingDetail,
} from "@/lib/api/job-postings"
const api = vi.hoisted(() => ({
  getJobPostingQueueCounts: vi.fn(),
  getPostingDetail: vi.fn(),
  listJobPostingsForQueue: vi.fn(),
}))
vi.mock("@/lib/api/job-postings", async (original) => ({ ...(await original()), ...api }))
afterEach(cleanup)
beforeEach(() => {
  vi.clearAllMocks()
  vi.stubGlobal("matchMedia", vi.fn(() => ({
    matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn(),
  })))
  api.getJobPostingQueueCounts.mockResolvedValue({
    inbox: 1, interested: 1, preparation: 0, applied: 0, archive: 0,
    all: 2, newInbox: 1, reviewInbox: 0,
  })
})
describe("Postings workspace lifecycle", () => {
  it("binds list completion to generation and queue identity across failure and empty states", async () => {
    const inbox = deferred<JobPosting[]>()
    api.listJobPostingsForQueue
      .mockReturnValueOnce(inbox.promise)
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce([])
    const view = renderWorkspace("/postings/inbox")
    view.rerender(workspace("/postings/interested"))
    await waitFor(() => expect(screen.getByTestId("list")).toHaveTextContent("failed:"))
    view.rerender(workspace("/postings/preparation"))
    await waitFor(() => expect(screen.getByTestId("list")).toHaveTextContent("ready:"))
    await act(() => inbox.resolve([posting(1)]))
    expect(screen.getByTestId("list")).toHaveTextContent("ready:")
  })
  it("guards stale Detail reconciliation and reuses the loaded active operation", async () => {
    const first = deferred<JobPostingDetail>(), second = deferred<JobPostingDetail>()
    api.listJobPostingsForQueue.mockResolvedValue([posting(1), posting(2)])
    api.getPostingDetail.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    renderWorkspace("/postings/inbox")
    await waitFor(() => expect(screen.getByTestId("list")).toHaveTextContent("1,2"))
    click(1); click(2)
    await act(() => second.resolve(detail(posting(2, { readState: "read" }))))
    await act(() => first.resolve(detail(posting(1, { readState: "read" }))))
    expect(screen.getByTestId("detail")).toHaveTextContent("loaded:2")
    expect(screen.getByTestId("reads")).toHaveTextContent("unread,read")
    click(2)
    expect(api.getPostingDetail).toHaveBeenCalledTimes(2)
  })
  it("invalidates pending Detail work on unmount", async () => {
    const pending = deferred<JobPostingDetail>()
    api.listJobPostingsForQueue.mockResolvedValue([posting(1)])
    api.getPostingDetail.mockReturnValue(pending.promise)
    const view = renderWorkspace("/postings/inbox")
    await waitFor(() => expect(screen.getByTestId("list")).toHaveTextContent("1"))
    click(1)
    view.unmount()
    await act(() => pending.resolve(detail(posting(1, { readState: "read" }))))
    expect(api.getJobPostingQueueCounts).toHaveBeenCalledTimes(1)
  })
  it("reconciles read state and Counts after successful opening from all", async () => {
    api.listJobPostingsForQueue.mockResolvedValue([posting(1)])
    api.getPostingDetail.mockResolvedValue(detail(posting(1, { readState: "read" })))
    renderWorkspace("/postings/all")
    await waitFor(() => expect(screen.getByTestId("list")).toHaveTextContent("1"))
    click(1)
    await waitFor(() => expect(screen.getByTestId("detail")).toHaveTextContent("loaded:1"))
    expect(screen.getByRole("link", { name: /Inbox 1/ })).toBeInTheDocument()
    expect(screen.getByTestId("reads")).toHaveTextContent("read")
    expect(api.getJobPostingQueueCounts).toHaveBeenCalledTimes(2)
  })
  it("reconciles authoritative state after an after-read failure", async () => {
    api.listJobPostingsForQueue
      .mockResolvedValueOnce([posting(1)])
      .mockResolvedValueOnce([posting(1, { readState: "read" })])
    api.getPostingDetail.mockRejectedValue(
      new PostingTransportError("after_read", "failed after mark-read", 1),
    )
    renderWorkspace("/postings/inbox")
    await waitFor(() => expect(screen.getByTestId("list")).toHaveTextContent("1"))
    click(1)
    await waitFor(() => expect(screen.getByTestId("detail")).toHaveTextContent("failed:1"))
    await waitFor(() => expect(screen.getByTestId("reads")).toHaveTextContent("read"))
    expect(api.listJobPostingsForQueue).toHaveBeenCalledTimes(2)
    expect(api.getJobPostingQueueCounts).toHaveBeenCalledTimes(2)
  })
})
function Probe() {
  const state = usePostingsList()
  return <>
    <div data-testid="list">{state.listLoading ? "loading" : state.listError ? "failed" : "ready"}:{state.postings.map(({ id }) => id).join(",")}</div>
    <div data-testid="reads">{state.postings.map(({ readState }) => readState).join(",")}</div>
    <div data-testid="detail">{state.detailState.status}{"postingId" in state.detailState ? `:${state.detailState.postingId}` : ""}</div>
    {[1, 2].map((id) => <button key={id} onClick={() => state.selectPosting(id)}>select-{id}</button>)}
  </>
}
function workspace(pathname: string) {
  return <PostingsWorkspaceProvider pathname={pathname}><SidebarProvider>
    <PostingsSidebar pathname={pathname} /><Probe />
  </SidebarProvider></PostingsWorkspaceProvider>
}
function renderWorkspace(pathname: string) { return render(workspace(pathname)) }
function click(id: number) { act(() => screen.getByRole("button", { name: `select-${id}` }).click()) }
function posting(id: number, overrides: Partial<JobPosting> = {}): JobPosting {
  const occurrence = {
    id: id * 10, sourceKey: "acme", sourceNameSnapshot: "Acme Careers",
    identity: { kind: "provider_posting_id" as const, value: `job-${id}` },
    providerUrl: `https://example.test/jobs/${id}`, postingMeta: {},
    firstSeenAt: "2026-07-05T10:00:00.000Z", lastSeenAt: "2026-07-05T11:00:00.000Z",
  }
  return {
    id, title: "Engineer", company: "Acme", locations: ["Berlin"], descriptionText: null,
    readState: "unread", interestState: "undecided", preparationState: "not_started",
    applicationState: "not_applied", primaryQueue: "inbox",
    firstSeenAt: occurrence.firstSeenAt, lastSeenAt: occurrence.lastSeenAt,
    createdAt: occurrence.firstSeenAt, updatedAt: occurrence.firstSeenAt,
    primaryOccurrence: occurrence, occurrences: [occurrence], ...overrides,
  }
}
function detail(value: JobPosting): JobPostingDetail {
  return { ...value, descriptionState: { status: "loaded", text: "Description", diagnostics: [] } }
}
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => { resolve = done })
  return { promise, resolve }
}
