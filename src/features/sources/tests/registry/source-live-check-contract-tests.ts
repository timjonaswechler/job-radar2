import assert from "node:assert/strict";

import {
  sourceLiveCheckActionsForSource,
  sourceLiveCheckDisplayModel,
} from "@/features/sources/view-model/source-live-check-model";
import type {
  CheckReport,
  SourceLiveCheckReportStatus,
} from "@/lib/api/sources";

const liveCheckReport: CheckReport = {
  schemaVersion: 1,
  kind: "source_live_check",
  subject: { type: "source", key: "acme" },
  checkedAt: "2026-07-07T13:00:00Z",
  logicVersion: "source-live-check/v1",
  result: "passed",
  fingerprints: [{ kind: "source_document", sha256: "source-sha" }],
  diagnostics: [],
  details: {
    sourceStatusAtCheck: "draft",
    liveCheckState: "live_check_passed",
    accessPathKey: "boards_api",
    candidateCount: 2,
    detailChecked: true,
    detailPassed: true,
  },
};

const passedModel = sourceLiveCheckDisplayModel(liveCheckStatus({}));
assert.equal(passedModel.displayState, "passed");
assert.equal(passedModel.displayLabel, "Live-Prüfung bestanden");
assert.equal(passedModel.reportStateLabel, "Frisch");
assert.equal(passedModel.reportResultLabel, "Bestanden");
assert.deepEqual(passedModel.diagnostics, []);

const failedModel = sourceLiveCheckDisplayModel(
  liveCheckStatus({
    report: {
      ...liveCheckReport,
      result: "failed",
      details: { ...liveCheckReport.details, liveCheckState: "live_check_failed" },
      diagnostics: [
        {
          category: "runtime",
          code: "source_live_check.activation_blocked",
          message: "Activation blocked",
          severity: "error",
          path: "/",
          details: {
            sourceKey: "acme",
            currentStatus: "draft",
            requestedStatus: "active",
            liveCheckResult: "failed",
          },
        },
      ],
    },
  }),
);
assert.equal(failedModel.displayState, "failed");
assert.equal(failedModel.diagnostics[0]?.code, "source_live_check.activation_blocked");

const staleModel = sourceLiveCheckDisplayModel(
  liveCheckStatus({
    state: "stale",
    freshness: {
      state: "stale",
      staleFingerprints: [
        { kind: "source_document", reason: "changed_fingerprint_sha256" },
      ],
    },
  }),
);
assert.equal(staleModel.displayState, "stale");
assert.equal(staleModel.staleFingerprints[0]?.kind, "source_document");
assert.equal(sourceLiveCheckDisplayModel(null).displayState, "unknown");
assert.equal(sourceLiveCheckDisplayModel({ state: "unknown" }).displayLabel, "Unbekannt");
for (const label of [passedModel.displayLabel, failedModel.displayLabel, staleModel.displayLabel]) {
  assert.equal(label.toLocaleLowerCase("de").includes("verifiziert"), false);
}

assert.deepEqual(sourceLiveCheckActionsForSource("active").map((action) => action.kind), [
  "check",
]);
assert.deepEqual(sourceLiveCheckActionsForSource("draft").map((action) => action.kind), [
  "check",
  "check_and_activate",
]);
assert.deepEqual(sourceLiveCheckActionsForSource("disabled").map((action) => action.kind), [
  "check",
  "check_and_reactivate",
]);
assert.equal(sourceLiveCheckActionsForSource("draft")[1]?.label, "Prüfen & Aktivieren");
assert.equal(
  sourceLiveCheckActionsForSource("disabled")[1]?.label,
  "Prüfen & Reaktivieren",
);

function liveCheckStatus(
  overrides: Partial<SourceLiveCheckReportStatus>,
): SourceLiveCheckReportStatus {
  return {
    state: "fresh",
    report: liveCheckReport,
    freshness: { state: "fresh", staleFingerprints: [] },
    ...overrides,
  };
}
