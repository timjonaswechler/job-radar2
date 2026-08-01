import { describe, expect, it } from "vitest";
import { decodeDetectionOutcome } from "./detection";

const usage = {
  strategyAttempts: 1,
  requests: 0,
  producedItems: 1,
  durationMs: 2,
  pages: 0,
  browserActions: 0,
  fanOut: 0,
  responseBytes: 0,
  browserRenderedBytes: 0,
};

const proposal = {
  profileKey: "greenhouse",
  profileName: "Greenhouse",
  recommendedAccessPathKey: "boards_api",
  recommendedAccessPathName: "Boards API",
  sourceConfig: { boardSlug: "example" },
  keyCandidates: ["example"],
  nameCandidates: ["Example"],
  captures: { boardSlug: "example" },
  evidence: [{
    kind: "url",
    message: "URL matched",
    descriptorPath: "/detection/strategies/0",
  }],
  supportLevel: "stable",
  provenance: {
    captures: { boardSlug: [{ strategyKey: "url", schemaPath: "/detection/strategies/0" }] },
    sourceConfig: {},
    recommendation: [],
    evidence: [[]],
  },
};

const outcome = {
  status: "matched",
  proposals: [proposal],
  unsupportedProfiles: [],
  profileDiagnostics: [],
  diagnostics: [],
  report: { usage, completion: { type: "accepted" } },
};

describe("Profile Detection transport", () => {
  it("decodes the intentional host projection", () => {
    expect(decodeDetectionOutcome(outcome)).toEqual(outcome);
  });

  it.each([
    ["engine attempts", { ...outcome, attempts: [] }],
    ["unknown status", { ...outcome, status: "partial" }],
    ["matched without one proposal", { ...outcome, proposals: [] }],
    ["ambiguous without multiple proposals", { ...outcome, status: "ambiguous" }],
    ["unsupported without an unsupported Profile", { ...outcome, status: "unsupported", proposals: [] }],
    ["coerced status", { ...outcome, status: ["matched"] }],
    ["malformed proposal", { ...outcome, proposals: [{ ...proposal, profileKey: 42 }] }],
    ["malformed Profile diagnostics", { ...outcome, profileDiagnostics: [{ code: "broken" }] }],
    ["incomplete usage", { ...outcome, report: { ...outcome.report, usage: { requests: 1 } } }],
    ["unknown completion", { ...outcome, report: { ...outcome.report, completion: { type: "retried" } } }],
    ["coerced completion", { ...outcome, report: { ...outcome.report, completion: { type: ["accepted"] } } }],
    ["unexpected projection field", { ...outcome, internalState: {} }],
    ["unknown exhaustion dimension", {
      ...outcome,
      report: { ...outcome.report, completion: {
        type: "budget_exhausted",
        exhaustion: { dimension: "retry_count", requested: 1, remaining: 0, limitSources: ["backend"] },
      } },
    }],
  ])("rejects %s", (_label, value) => {
    expect(() => decodeDetectionOutcome(value)).toThrow(/invalid|forbidden/);
  });
});
