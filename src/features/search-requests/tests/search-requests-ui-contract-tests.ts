import assert from "node:assert/strict";

import {
  buildSearchRequestInput,
  createEmptySearchRequestForm,
  searchRequestFormFromRequest,
} from "@/features/search-requests/model/search-request-form-model";
import { createSearchRunDiagnosticViewModels } from "@/features/search-requests/model/search-run-diagnostics";
import type { SearchRequest } from "@/lib/api/search-requests";
import type { StructuredDiagnostic } from "@/lib/api/sources";

const newForm = createEmptySearchRequestForm(42);
assert.equal(newForm.radiusKmText, "42");
assert.equal(buildSearchRequestInput(newForm).input?.radiusKm, 42);

const savedRadiusForm = searchRequestFormFromRequest(searchRequest({ radiusKm: 15 }), 42);
assert.equal(savedRadiusForm.radiusKmText, "15");
assert.equal(buildSearchRequestInput(savedRadiusForm).input?.radiusKm, 15);

const unsavedRadiusForm = searchRequestFormFromRequest(searchRequest({ radiusKm: null }), 42);
assert.equal(unsavedRadiusForm.radiusKmText, "42");
assert.equal(buildSearchRequestInput(unsavedRadiusForm).input?.radiusKm, 42);

const missingRadiusDiagnostic = structuredDiagnostic({
  code: "location_filter_not_applied_missing_radius_km",
  message: "backend message",
  path: "/radiusKm",
  severity: "warning",
});
const [missingRadiusViewModel] = createSearchRunDiagnosticViewModels([
  missingRadiusDiagnostic,
]);
assert.equal(missingRadiusViewModel.title, "Standortfilter nicht angewendet");
assert.match(missingRadiusViewModel.message, /keinen gespeicherten Radius/);
assert.equal(missingRadiusViewModel.code, "location_filter_not_applied_missing_radius_km");

function searchRequest(overrides: Partial<SearchRequest> = {}): SearchRequest {
  return {
    id: 1,
    status: "draft",
    includeRules: [],
    excludeRules: [],
    locations: ["Mainz"],
    radiusKm: null,
    sourceKeys: [],
    validationError: null,
    lastRunAt: null,
    lastRunStatus: null,
    lastRunError: null,
    createdAt: "2026-07-09T00:00:00Z",
    updatedAt: "2026-07-09T00:00:00Z",
    ...overrides,
  };
}

function structuredDiagnostic(
  overrides: Partial<StructuredDiagnostic> = {},
): StructuredDiagnostic {
  return {
    category: "runtime",
    code: "test_diagnostic",
    message: "Diagnostic message",
    severity: "info",
    path: "",
    ...overrides,
  };
}
