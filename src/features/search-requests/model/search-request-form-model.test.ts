import { describe, expect, it } from "vitest"

import {
  buildSearchRequestInput,
  createEmptySearchRequestForm,
  searchRequestFormFromRequest,
} from "./search-request-form-model"
import type { SearchRequest } from "@/lib/api/search-requests"

describe("Search Request form radius ownership", () => {
  it("applies the preference radius only when creating a request", () => {
    const form = createEmptySearchRequestForm(42)

    expect(form.radiusKmText).toBe("42")
    expect(buildSearchRequestInput(form).input?.radiusKm).toBe(42)
  })

  it("preserves an authored null radius while editing", () => {
    const form = searchRequestFormFromRequest(request({ radiusKm: null }))

    expect(form.radiusKmText).toBe("")
    expect(buildSearchRequestInput(form).input?.radiusKm).toBeNull()
  })

  it("preserves an authored numeric radius while editing", () => {
    const form = searchRequestFormFromRequest(request({ radiusKm: 15 }))

    expect(form.radiusKmText).toBe("15")
    expect(buildSearchRequestInput(form).input?.radiusKm).toBe(15)
  })
})

function request(overrides: Partial<SearchRequest> = {}): SearchRequest {
  return {
    id: 1,
    status: "draft",
    includeRules: [],
    excludeRules: [],
    locations: ["Mainz"],
    radiusKm: null,
    sourceKeys: [],
    validationIssues: [],
    lastRunAt: null,
    lastRunStatus: null,
    lastRunError: null,
    createdAt: "2026-07-09T00:00:00Z",
    updatedAt: "2026-07-09T00:00:00Z",
    ...overrides,
  }
}
