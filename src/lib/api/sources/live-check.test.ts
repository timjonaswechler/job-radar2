import { describe, expect, it } from "vitest";

import {
  decodeAdmissionOutcome,
  decodeRunOutcome,
  decodeSourceLiveCheckError,
  decodeSourceLiveCheckReportStatus,
  SourceLiveCheckCommandError,
} from "./live-check";

const report = {
  schemaVersion: 1,
  kind: "source_live_check",
  subject: { type: "source", key: "example_source" },
  checkedAt: "2025-01-02T03:04:05Z",
  logicVersion: "source-live-check/v2",
  result: "passed",
  fingerprints: [{
    kind: "source_behavior",
    sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    reference: "source_config",
  }],
  diagnostics: [],
  details: { liveCheckState: "live_check_passed" },
};

const source = {
  origin: "custom",
  fileName: "example_source.json",
  document: {
    schemaVersion: 3,
    key: "example_source",
    name: "Example",
    status: "active",
    sourceConfig: {},
    selectedAccessPath: {
      type: "profile_access_path",
      profileKey: "example_jobs",
      pathKey: "json_feed",
    },
  },
  validationState: {
    sourceKey: "example_source",
    state: "valid",
    canCompile: true,
    canExecute: true,
  },
};

describe("Source Live Check transport", () => {
  it("decodes operation-specific run and admission outcomes", () => {
    expect(decodeRunOutcome({ report }).report).toEqual(report);
    expect(
      decodeAdmissionOutcome({
        type: "checked",
        report: { ...report, result: "failed" },
      }).type,
    ).toBe("checked");
    expect(
      decodeAdmissionOutcome({ type: "activated", report, source }).type,
    ).toBe("activated");
  });

  it("decodes typed operation errors", () => {
    const error = decodeSourceLiveCheckError({
      kind: "stale_generation",
      message: "Source changed during checking",
    });
    expect(error).toBeInstanceOf(SourceLiveCheckCommandError);
    expect((error as SourceLiveCheckCommandError).kind).toBe("stale_generation");
    expect(error.message).toBe("Source changed during checking");
  });

  it("decodes missing report status and rejects malformed reports", () => {
    expect(
      decodeSourceLiveCheckReportStatus({
        state: "unknown",
        report: null,
        freshness: null,
      }).state,
    ).toBe("unknown");
    expect(() =>
      decodeRunOutcome({ report: { ...report, result: "maybe" } }),
    ).toThrow(/invalid Source Live Check/);
    expect(() =>
      decodeSourceLiveCheckReportStatus({
        state: "fresh",
        report,
        freshness: { state: "stale", staleFingerprints: [] },
      }),
    ).toThrow(/invalid Source Live Check/);
  });
});
