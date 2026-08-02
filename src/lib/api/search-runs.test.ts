import { beforeEach, describe, expect, it, vi } from "vitest"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/core", () => ({ invoke }))

import { parseSearchRunOutcome, runSearchRequest } from "./search-runs"

beforeEach(() => invoke.mockReset())

describe("Search Run transport", () => {
  it("decodes a Search Run with Source Run resolution", () => {
    expect(parseSearchRunOutcome(searchRunWire())).toEqual(searchRunWire())
  })

  it("drops obsolete full Posting bodies from the decoded Outcome", () => {
    expect(parseSearchRunOutcome({ ...searchRunWire(), postings: [{ title: "obsolete" }] }))
      .not.toHaveProperty("postings")
  })

  it("starts a Search Run and validates its Background Task snapshot", async () => {
    invoke.mockResolvedValueOnce({
      taskId: "task-1",
      kind: "search_run",
      state: "queued",
      progress: null,
      result: null,
      error: null,
      diagnostics: [],
    })

    await expect(runSearchRequest(7)).resolves.toMatchObject({
      taskId: "task-1",
      state: "queued",
    })
    expect(invoke).toHaveBeenCalledWith("run_search_request", { id: 7 })
  })

  it.each([
    ["unknown run status", { status: "running" }],
    ["malformed Source Runs", { sourceRuns: "invalid" }],
    ["invalid matched Posting count", { matchedPostingCount: -1 }],
    ["malformed Diagnostics", { diagnostics: [{ severity: "maybe" }] }],
  ])("rejects %s", (_case, override) => {
    expect(parseSearchRunOutcome({ ...searchRunWire(), ...override })).toBeNull()
  })
})

function searchRunWire() {
  return {
    searchRequestId: 1,
    status: "completed_with_errors" as const,
    generatedAt: "2026-08-01T12:00:00Z",
    diagnostics: [],
    sourceRuns: [{
      sourceKey: "fixture_source",
      sourceName: "Fixture Source",
      status: "completed" as const,
      resolution: {
        completion: { type: "complete" as const },
        counts: {
          discovered: 1,
          processed: 1,
          finalized: 1,
          rejected: 0,
          unresolved: 0,
          failed: 0,
          budgetSkipped: 0,
        },
        remaining: null,
        usage: {
          strategyAttempts: 1,
          requests: 1,
          producedItems: 1,
          durationMs: 5,
          pages: 1,
          browserActions: 0,
          fanOut: 0,
          responseBytes: 100,
          browserRenderedBytes: 0,
        },
        candidateDiagnostics: {
          countsByCode: {},
          samples: [],
          sampleLimit: 10,
          candidateDiagnosticsOmitted: 0,
        },
      },
      diagnostics: [],
      error: null,
    }],
    matchedPostingCount: 1,
  }
}
