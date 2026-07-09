import assert from "node:assert/strict";

import { SourceRegistryTab } from "@/features/sources/registry/source/source-registry-tab";
import {
  countOrigins,
  countSourceStatuses,
  createSourceGridRows,
  filterSourceGridRows,
} from "@/features/sources/view-model/source-grid-model";
import { resolveSource } from "@/features/sources/view-model/registry-resolution";
import type {
  RegistrySource,
  RegistrySourceProfile,
} from "@/lib/api/sources";

assert.equal(typeof SourceRegistryTab, "function");

const profile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/greenhouse.json",
  document: {
    schemaVersion: 2,
    key: "greenhouse",
    name: "Greenhouse",
    kind: "recruiting_system",
    support: { level: "stable" },
    sourceConfigSchema: {
      type: "object",
      required: ["boardSlug"],
      properties: { boardSlug: { type: "string" } },
    },
    accessPaths: [
      {
        key: "boards_api",
        name: "Boards API",
        postingDiscovery: { strategies: [{ key: "jobs_api" }] },
        postingDetail: { strategies: [{ key: "detail_api" }] },
      },
    ],
  },
};
const profilesByKey = new Map([[profile.document.key, profile]]);

const source: RegistrySource = {
  origin: "custom",
  path: "sources/acme.json",
  document: {
    schemaVersion: 2,
    key: "acme",
    name: "ACME",
    status: "active",
    sourceConfig: { boardSlug: "acme" },
    selectedAccessPath: {
      type: "profile_access_path",
      profileKey: "greenhouse",
      pathKey: "boards_api",
    },
  },
  validationState: {
    sourceKey: "acme",
    state: "valid",
    canCompile: true,
    canExecute: true,
    diagnostics: [],
  },
};

const sourceOwnedSource: RegistrySource = {
  origin: "custom",
  path: "sources/one_off.json",
  document: {
    schemaVersion: 2,
    key: "one_off",
    name: "One Off",
    status: "draft",
    sourceConfig: { startUrl: "https://example.test/jobs" },
    sourceSupport: { level: "experimental" },
    selectedAccessPath: {
      type: "source_owned_access_path",
      key: "html_jobs",
      name: "HTML jobs",
      sourceConfigSchema: { type: "object" },
      postingDiscovery: { strategies: [{ key: "html" }] },
    },
  },
  validationState: {
    sourceKey: "one_off",
    state: "invalid",
    canCompile: false,
    canExecute: false,
    diagnostics: [
      {
        category: "source_validation",
        code: "source_validation_failed",
        message: "Source cannot compile",
        severity: "error",
        path: "",
        details: { sourceKey: "one_off" },
      },
    ],
  },
};

const missingProfileSource: RegistrySource = {
  ...source,
  path: "sources/missing_profile.json",
  document: {
    ...source.document,
    key: "missing_profile_source",
    name: "Missing Profile Source",
    selectedAccessPath: {
      type: "profile_access_path",
      profileKey: "missing_profile",
      pathKey: "jobs",
    },
  },
  validationState: {
    sourceKey: "missing_profile_source",
    state: "unknown",
    canCompile: false,
    canExecute: false,
    diagnostics: [],
  },
};

const missingAccessPathSource: RegistrySource = {
  ...source,
  path: "sources/missing_path.json",
  document: {
    ...source.document,
    key: "missing_path_source",
    name: "Missing Access Path Source",
    status: "disabled",
    selectedAccessPath: {
      type: "profile_access_path",
      profileKey: "greenhouse",
      pathKey: "missing_path",
    },
  },
  validationState: {
    sourceKey: "missing_path_source",
    state: "unknown",
    canCompile: false,
    canExecute: false,
    diagnostics: [],
  },
};

const rows = createSourceGridRows(
  [source, sourceOwnedSource, missingProfileSource, missingAccessPathSource],
  profilesByKey,
  new Map([
    [
      "missing_profile_source",
      [
        {
          category: "registry",
          code: "missing_source_profile",
          message: "Selected Source Profile is missing",
          severity: "error",
          path: "/selectedAccessPath/profileKey",
          details: { sourceKey: "missing_profile_source" },
        },
      ],
    ],
    [
      "missing_path_source",
      [
        {
          category: "registry",
          code: "missing_access_path",
          message: "Selected Access Path is missing",
          severity: "error",
          path: "/selectedAccessPath/pathKey",
          details: { sourceKey: "missing_path_source" },
        },
      ],
    ],
  ]),
);
assert.deepEqual(
  rows.map((row) => [row.key, row.health, row.ownDiagnosticsCount, row.dependencyDiagnosticsCount]),
  [
    ["acme", "valid", 0, 0],
    ["one_off", "invalid", 1, 0],
    ["missing_profile_source", "dependency_warning", 0, 1],
    ["missing_path_source", "dependency_warning", 0, 1],
  ],
);
assert.equal(rows[0]?.supportLabel, "Stabil");
assert.equal(rows[0]?.validationStateLabel, "Valide");
assert.equal(rows[0]?.capabilitiesSummary, "postingDiscovery, postingDetail");
assert.equal(rows[0]?.profileLabel, "greenhouse / boards_api");
for (const removedTerm of removedSourceProfileTerms()) {
  assert.equal((rows[0]?.searchText ?? "").includes(removedTerm), false);
}
assert.equal(rows[1]?.supportLabel, "Experimentell");
assert.equal(rows[1]?.accessPathLabel, "Source-owned · html_jobs");
assert.equal(rows[2]?.supportLabel, "—");
assert.deepEqual(countSourceStatuses(rows), { draft: 1, active: 2, disabled: 1 });
assert.deepEqual(countOrigins(rows), { built_in: 0, custom: 4 });
assert.deepEqual(
  filterSourceGridRows(rows, {
    searchQuery: "missing",
    statuses: ["active"],
    origins: [],
    diagnosticsOnly: true,
  }).map((row) => row.key),
  ["missing_profile_source"],
);

const resolution = resolveSource(source, profilesByKey);
assert.equal(resolution.profileAccessPath?.key, "boards_api");
assert.equal(resolution.supportLevel, "stable");
assert.deepEqual(resolution.capabilities, ["postingDiscovery", "postingDetail"]);
const sourceOwnedResolution = resolveSource(sourceOwnedSource, profilesByKey);
assert.equal(sourceOwnedResolution.profile, null);
assert.equal(sourceOwnedResolution.sourceOwnedAccessPath?.key, "html_jobs");
assert.deepEqual(sourceOwnedResolution.effectiveSourceConfigSchema, { type: "object" });
assert.equal(sourceOwnedResolution.supportLevel, "experimental");
assert.deepEqual(sourceOwnedResolution.capabilities, ["postingDiscovery"]);
const missingAccessPathResolution = resolveSource(
  missingAccessPathSource,
  profilesByKey,
);
assert.equal(missingAccessPathResolution.profile?.document.key, "greenhouse");
assert.equal(missingAccessPathResolution.profileAccessPath, null);
assert.deepEqual(missingAccessPathResolution.capabilities, []);

function removedSourceProfileTerms() {
  return [
    "adap" + "ter" + "Key",
    "inven" + "tory",
    "source" + "_specific",
    "Source" + "Specific",
  ];
}
