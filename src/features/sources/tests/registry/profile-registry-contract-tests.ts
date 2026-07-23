import assert from "node:assert/strict";

import { minimalDiscoveryStrategy } from "@/features/sources/tests/support/profile-dsl";

import { profileGridColumns } from "@/features/sources/registry/profile/profile-grid-columns";
import { ProfileRegistryTab } from "@/features/sources/registry/profile/profile-registry-tab";
import {
  countProfileKinds,
  countProfileOrigins,
  createProfileGridRows,
  filterProfileGridRows,
} from "@/features/sources/view-model/profile-grid-model";
import type { RegistrySourceProfile } from "@/lib/api/sources";

assert.equal(typeof ProfileRegistryTab, "function");
assert.equal(
  profileGridColumns.some((column) => column.id === "supportEvidenceSummary"),
  true,
);
assert.equal(
  profileGridColumns.some((column) => column.id === "detectionEvidenceSummary"),
  true,
);

const stableProfile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/greenhouse.json",
  document: {
    schemaVersion: 3,
    key: "greenhouse",
    name: "Greenhouse",
    kind: "recruiting_system",
    support: { level: "stable" },
    accessPaths: [
      {
        key: "boards_api",
        name: "Boards API",
        discovery: { policy: { type: "first_accepted" }, strategies: [minimalDiscoveryStrategy("jobs_api")] },
      },
    ],
  },
};
const warningProfile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/warning.json",
  document: {
    ...stableProfile.document,
    key: "warning_profile",
    name: "Warning Profile",
    kind: "generic",
    support: { level: "best_effort" },
    accessPaths: [],
  },
};
const errorProfile: RegistrySourceProfile = {
  origin: "custom",
  path: "profiles/error.json",
  document: {
    ...stableProfile.document,
    key: "error_profile",
    name: "Error Profile",
    kind: "generic",
    support: { level: "experimental" },
    diagnostics: [
      {
        category: "schema",
        code: "profile_schema_error",
        message: "Profile schema is invalid",
        severity: "error",
        path: "/accessPaths/0",
        details: { sourceProfileKey: "error_profile" },
      },
    ],
    accessPaths: [],
  },
};

const rows = createProfileGridRows(
  [stableProfile, warningProfile, errorProfile],
  new Map([
    [
      "warning_profile",
      [
        {
          category: "registry",
          code: "profile_known_issue",
          message: "Profile has a known issue",
          severity: "warning",
          path: "/support/knownIssues/0",
          details: { sourceProfileKey: "warning_profile" },
        },
      ],
    ],
  ]),
);
assert.deepEqual(
  rows.map((row) => [row.key, row.health, row.ownDiagnosticsCount, row.dependencyDiagnosticsCount]),
  [
    ["greenhouse", "valid", 0, 0],
    ["warning_profile", "dependency_warning", 1, 0],
    ["error_profile", "invalid", 1, 0],
  ],
);
assert.deepEqual(countProfileKinds(rows), {
  recruiting_system: 1,
  job_portal: 0,
  website_family: 0,
  career_site: 0,
  generic: 2,
});
assert.deepEqual(countProfileOrigins(rows), { built_in: 2, custom: 1 });
assert.deepEqual(
  filterProfileGridRows(rows, {
    searchQuery: "warning",
    kinds: ["generic"],
    origins: ["built_in"],
    diagnosticsOnly: true,
  }).map((row) => row.key),
  ["warning_profile"],
);

const evidenceProfile: RegistrySourceProfile = {
  origin: "custom",
  path: "profiles/evidence.json",
  document: {
    ...stableProfile.document,
    key: "evidence_profile",
    name: "Evidence Profile",
    support: {
      level: "stable",
      evidence: [
        { kind: "smoke", reference: "https://jobs.example.test" },
        { kind: "manual_review", reference: "review-2026-07" },
        { kind: "schema_check", reference: "schema-validation" },
      ],
    },
    detection: {
      policy: { type: "all_required" },
      strategies: [{ type: "url", key: "input_url", input: { type: "absolute_url" } }],
      evidence: [
        { kind: "url", message: "Matched board URL" },
        { kind: "http", message: "HTTP marker matched" },
      ],
    },
    accessPaths: [],
  },
};
const [evidenceRow] = createProfileGridRows([evidenceProfile], new Map());
assert.deepEqual(evidenceRow?.supportEvidenceLabels, [
  "Smoke",
  "Manual Review",
  "Schema Check",
]);
assert.equal(evidenceRow?.supportEvidenceSummary, "Smoke, Manual Review, Schema Check");
assert.deepEqual(evidenceRow?.detectionEvidenceLabels, ["URL", "HTTP"]);
assert.equal(evidenceRow?.detectionEvidenceKinds.includes("url"), true);
assert.equal((evidenceRow?.supportEvidenceKinds as string[]).includes("url"), false);
