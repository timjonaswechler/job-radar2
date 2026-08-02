import { beforeEach, describe, expect, it, vi } from "vitest"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/core", () => ({ invoke }))

import {
  composeSearchRequestView,
  createSearchRequest,
  decodeSearchRequestError,
  deleteSearchRequest,
  getSearchRequest,
  listSearchRequests,
  parseSearchRequest,
  updateSearchRequest,
  SearchRequestCommandError,
  type SearchRequestRecord,
} from "./search-requests"

beforeEach(() => invoke.mockReset())

describe("Search Request transport", () => {
  it("decodes authored lifecycle, derived Validation, and the latest Search Run projection", () => {
    const request = parseSearchRequest(searchRequestWire())

    expect(request).toEqual({
      id: 1,
      status: "draft",
      includeRules: [{ target: "title", kind: "text", value: "Physik" }],
      excludeRules: [],
      locations: ["Mainz"],
      radiusKm: null,
      sourceKeys: ["fixture_source"],
      validationIssues: [{
        code: "duplicate_source_key",
        path: "/sourceKeys/1",
        message: "Remove the duplicate Source key.",
      }],
      lastRunAt: "2026-08-01T12:00:00Z",
      lastRunStatus: "completed_with_errors",
      lastRunError: "one Source failed",
      createdAt: "2026-07-09T00:00:00Z",
      updatedAt: "2026-07-10T00:00:00Z",
    })
  })

  it.each([
    ["persisted invalid lifecycle", { status: "invalid" }],
    ["malformed Validation", { validationIssues: "invalid" }],
    ["unknown Validation issue", { validationIssues: [{ code: "unknown", path: "", message: "bad" }] }],
    ["unsafe radius", { radiusKm: Number.MAX_SAFE_INTEGER + 1 }],
    ["malformed latest summary", { lastRunStatus: "running" }],
  ])("rejects %s", (_case, override) => {
    expect(parseSearchRequest({ ...searchRequestWire(), ...override })).toBeNull()
  })

  it("composes the flat host view from a request Record and latest-run summary", () => {
    const record: SearchRequestRecord = {
      id: 1,
      status: "draft",
      includeRules: [],
      excludeRules: [],
      locations: [],
      radiusKm: null,
      sourceKeys: [],
      validation: { issues: [] },
      createdAt: "2026-07-09T00:00:00Z",
      updatedAt: "2026-07-09T00:00:00Z",
    }

    expect(composeSearchRequestView(record, {
      at: "2026-08-01T12:00:00Z",
      status: "failed",
      error: "run failed",
    })).toMatchObject({
      validationIssues: [],
      lastRunAt: "2026-08-01T12:00:00Z",
      lastRunStatus: "failed",
      lastRunError: "run failed",
    })
  })

  it("validates every CRUD result at the Tauri seam", async () => {
    const input = {
      status: "draft" as const,
      includeRules: [],
      excludeRules: [],
      locations: [],
      radiusKm: null,
      sourceKeys: [],
    }
    invoke
      .mockResolvedValueOnce(searchRequestWire())
      .mockResolvedValueOnce([searchRequestWire()])
      .mockResolvedValueOnce(searchRequestWire())
      .mockResolvedValueOnce(searchRequestWire())
      .mockResolvedValueOnce(undefined)

    await expect(createSearchRequest(input)).resolves.toMatchObject({ id: 1 })
    await expect(listSearchRequests()).resolves.toHaveLength(1)
    await expect(getSearchRequest(1)).resolves.toMatchObject({ id: 1 })
    await expect(updateSearchRequest(1, input)).resolves.toMatchObject({ id: 1 })
    await expect(deleteSearchRequest(1)).resolves.toBeUndefined()
    expect(invoke.mock.calls.map(([command]) => command)).toEqual([
      "create_search_request",
      "list_search_requests",
      "get_search_request",
      "update_search_request",
      "delete_search_request",
    ])

    invoke.mockResolvedValueOnce({ ...searchRequestWire(), status: "invalid" })
    await expect(createSearchRequest(input)).rejects.toThrow(/invalid shape/i)
    invoke.mockResolvedValueOnce({ unexpected: true })
    await expect(deleteSearchRequest(1)).rejects.toThrow(/invalid shape/i)
  })

  it("projects typed CRUD error distinctions", async () => {
    for (const [kind, id] of [
      ["invalid_input", undefined],
      ["not_found", 1],
      ["busy", 1],
      ["corrupt_stored_row", 1],
      ["storage_unavailable", undefined],
      ["internal_invariant", undefined],
    ] as const) {
      const error = decodeSearchRequestError({
        kind,
        message: `fixture ${kind}`,
        ...(id === undefined ? {} : { id }),
      })
      expect(error).toBeInstanceOf(SearchRequestCommandError)
      expect(error).toMatchObject({ kind, id })
    }
    expect(decodeSearchRequestError({
      kind: "busy",
      message: "missing id",
    })).not.toBeInstanceOf(SearchRequestCommandError)
    expect(decodeSearchRequestError({
      kind: "invalid_input",
      message: "unexpected id",
      id: 1,
    })).not.toBeInstanceOf(SearchRequestCommandError)

    invoke.mockRejectedValueOnce({
      kind: "not_found",
      message: "search request 2 not found",
      id: 2,
    })
    await expect(getSearchRequest(2)).rejects.toMatchObject({
      kind: "not_found",
      id: 2,
    })
  })
})

function searchRequestWire() {
  return {
    id: 1,
    status: "draft",
    includeRules: [{ target: "title", kind: "text", value: "Physik" }],
    excludeRules: [],
    locations: ["Mainz"],
    radiusKm: null,
    sourceKeys: ["fixture_source"],
    validationIssues: [{
      code: "duplicate_source_key",
      path: "/sourceKeys/1",
      message: "Remove the duplicate Source key.",
    }],
    lastRunAt: "2026-08-01T12:00:00Z",
    lastRunStatus: "completed_with_errors",
    lastRunError: "one Source failed",
    createdAt: "2026-07-09T00:00:00Z",
    updatedAt: "2026-07-10T00:00:00Z",
  }
}
