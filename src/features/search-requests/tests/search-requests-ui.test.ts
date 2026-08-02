import assert from "node:assert/strict";

import {
  buildSearchRequestInput,
  createEmptySearchRequestForm,
  searchRequestFormFromRequest,
} from "@/features/search-requests/model/search-request-form-model";
import { createSearchRunDiagnosticViewModels } from "@/features/search-requests/model/search-run-diagnostics";
import {
  parseSearchRequest,
  type SearchRequest,
} from "@/lib/api/search-requests";
import type { StructuredDiagnostic } from "@/lib/api/sources";
import { test } from "vitest";

test("search requests ui contract", async () => {
  const newForm = createEmptySearchRequestForm(42);
  assert.equal(newForm.radiusKmText, "42");
  assert.equal(buildSearchRequestInput(newForm).input?.radiusKm, 42);

  const savedRadiusForm = searchRequestFormFromRequest(searchRequest({ radiusKm: 15 }));
  assert.equal(savedRadiusForm.radiusKmText, "15");
  assert.equal(buildSearchRequestInput(savedRadiusForm).input?.radiusKm, 15);

  const noRadiusForm = searchRequestFormFromRequest(searchRequest({ radiusKm: null }));
  assert.equal(noRadiusForm.radiusKmText, "");
  assert.equal(buildSearchRequestInput(noRadiusForm).input?.radiusKm, null);

  const maximumRadiusForm = createEmptySearchRequestForm(Number.MAX_SAFE_INTEGER);
  assert.equal(
    buildSearchRequestInput(maximumRadiusForm).input?.radiusKm,
    Number.MAX_SAFE_INTEGER,
  );
  maximumRadiusForm.radiusKmText = String(Number.MAX_SAFE_INTEGER + 1);
  assert.match(buildSearchRequestInput(maximumRadiusForm).errors[0], /höchstens/);

  const duplicateDraft = createEmptySearchRequestForm();
  duplicateDraft.sourceKeys = ["fixture_source", "fixture_source"];
  const duplicateDraftResult = buildSearchRequestInput(duplicateDraft);
  assert.deepEqual(duplicateDraftResult.input?.sourceKeys, [
    "fixture_source",
    "fixture_source",
  ]);
  assert.match(duplicateDraftResult.warnings[0], /doppelt.*entferne/i);

  duplicateDraft.status = "active";
  duplicateDraft.includeRules[0].value = "Physik";
  const duplicateActiveResult = buildSearchRequestInput(duplicateDraft);
  assert.equal(duplicateActiveResult.input, null);
  assert.match(duplicateActiveResult.errors[0], /doppelt.*entferne/i);

  const decoded = parseSearchRequest(searchRequest({
    validationIssues: [{
      code: "duplicate_source_key",
      path: "/sourceKeys/1",
      message: "Source key duplicates /sourceKeys/0; remove the duplicate entry.",
    }],
  }));
  assert.equal(decoded?.validationIssues[0].code, "duplicate_source_key");
  assert.equal(parseSearchRequest({ ...searchRequest(), status: "invalid" }), null);
  assert.equal(parseSearchRequest({ ...searchRequest(), validationIssues: "invalid" }), null);
  assert.equal(
    parseSearchRequest({ ...searchRequest(), radiusKm: Number.MAX_SAFE_INTEGER + 1 }),
    null,
  );

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
      validationIssues: [],
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
});
