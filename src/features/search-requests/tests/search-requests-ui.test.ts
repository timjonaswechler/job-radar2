import assert from "node:assert/strict";

import {
  buildSearchRequestInput,
  createEmptySearchRequestForm,
} from "@/features/search-requests/model/search-request-form-model";
import { createSearchRunDiagnosticViewModels } from "@/features/search-requests/model/search-run-diagnostics";
import type { StructuredDiagnostic } from "@/lib/api/sources";
import { test } from "vitest";

test("search requests ui contract", async () => {
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
