import assert from "node:assert/strict";

import { sourceDetectionOutcomeCopy } from "@/features/sources/add/source-detection-panel";
import {
  buildSourceDocument,
  detectedSourceFromProposal,
} from "@/features/sources/add/source-add-model";
import { sourceConfigSchemaMetadata } from "@/features/sources/add/source-config-schema";
import {
  createSourceGridRows,
  resolveSource,
} from "@/features/sources/view-model/registry-view-model";
import type {
  JsonValue,
  RegistrySource,
  RegistrySourceProfile,
  SourceDocument,
  SourceProposal,
} from "@/lib/api/sources";

const profile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/greenhouse.json",
  document: {
    schemaVersion: 2,
    key: "greenhouse",
    name: "Greenhouse",
    kind: "recruiting_system",
    support: { level: "verified" },
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

const profilesByKey = new Map([[profile.document.key, profile]]);

const sourceRows = createSourceGridRows(
  [source],
  profilesByKey,
  new Map(),
);

assert.equal(sourceRows[0]?.supportLabel, "Verifiziert");
assert.equal(sourceRows[0]?.validationStateLabel, "Valide");
assert.equal(sourceRows[0]?.capabilitiesSummary, "postingDiscovery, postingDetail");
assert.equal(sourceRows[0]?.profileLabel, "greenhouse / boards_api");
assert.equal(sourceRows[0]?.health, "valid");
for (const removedTerm of removedSourceProfileTerms()) {
  assert.equal((sourceRows[0]?.searchText ?? "").includes(removedTerm), false);
}

const resolution = resolveSource(source, profilesByKey);
assert.equal(resolution.profileAccessPath?.key, "boards_api");
assert.equal(resolution.supportLevel, "verified");
assert.deepEqual(resolution.capabilities, ["postingDiscovery", "postingDetail"]);

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

const sourceOwnedRows = createSourceGridRows(
  [sourceOwnedSource],
  profilesByKey,
  new Map(),
);
assert.equal(sourceOwnedRows[0]?.supportLabel, "Experimentell");
assert.equal(sourceOwnedRows[0]?.accessPathLabel, "Source-owned · html_jobs");
assert.equal(sourceOwnedRows[0]?.health, "invalid");

const proposal: SourceProposal = {
  profileKey: "greenhouse",
  profileName: "Greenhouse",
  recommendedAccessPathKey: "boards_api",
  recommendedAccessPathName: "Boards API",
  sourceConfig: { boardSlug: "acme" },
  keyCandidates: ["acme"],
  nameCandidates: ["ACME GmbH"],
  captures: { boardSlug: "acme" },
  evidence: [{ kind: "url", message: "Matched board URL" }],
  supportLevel: "verified",
};

const detected = detectedSourceFromProposal(proposal);
assert.deepEqual(detected, {
  profileKey: "greenhouse",
  pathKey: "boards_api",
  key: "acme",
  name: "ACME GmbH",
  sourceConfig: { boardSlug: "acme" },
});

const buildResult = buildSourceDocument({
  form: {
    name: detected?.name ?? "",
    key: detected?.key ?? "",
    status: "draft",
    profileKey: detected?.profileKey ?? "",
    pathKey: detected?.pathKey ?? "",
  },
  configEntries: [{ id: "boardSlug", key: "boardSlug", value: "acme" }],
  existingSourceKeys: new Set(),
  selectedProfile: profile,
  selectedAccessPath: profile.document.accessPaths[0] ?? null,
  schemaMetadata: sourceConfigSchemaMetadata(profile.document.sourceConfigSchema ?? {}),
});

assert.deepEqual(buildResult.errors, []);
assert.equal(buildResult.document?.schemaVersion, 2);
assert.equal(buildResult.document?.selectedAccessPath.type, "profile_access_path");
assert.deepEqual(buildResult.document?.sourceConfig, { boardSlug: "acme" });
assertNoV1SourceProfileFields(buildResult.document);

const failedDetectionCopy = sourceDetectionOutcomeCopy({
  status: "failed",
  diagnostics: [],
});
assert.equal(failedDetectionCopy.title, "Profilerkennung fehlgeschlagen");
assert.equal(failedDetectionCopy.description.includes("startUrl übernommen"), false);
assert.equal(
  failedDetectionCopy.description.includes("kein Konfigurationswert automatisch übernommen"),
  true,
);

const unsupportedDetectionCopy = sourceDetectionOutcomeCopy({
  status: "unsupported",
  unsupportedProfiles: [
    {
      profileKey: "known_ats",
      profileName: "Known ATS",
      supportLevel: "unsupported",
      captures: {},
      evidence: [{ kind: "url", message: "Known ATS URL" }],
    },
  ],
  diagnostics: [],
});
assert.equal(unsupportedDetectionCopy.title, "Kein ausführbares Profil verfügbar");
assert.equal(unsupportedDetectionCopy.description.includes("nicht unterstütztes Profil"), true);
assert.equal(unsupportedDetectionCopy.description.includes("startUrl übernommen"), true);

function assertNoV1SourceProfileFields(value: JsonValue | SourceDocument | null | undefined) {
  if (value === null || value === undefined) return;
  if (Array.isArray(value)) {
    value.forEach(assertNoV1SourceProfileFields);
    return;
  }
  if (typeof value === "object") {
    const keys = Object.keys(value);
    for (const removedTerm of removedSourceProfileTerms()) {
      assert.equal(keys.includes(removedTerm), false);
    }
    assert.equal(keys.includes("invalid"), false);
    for (const childValue of Object.values(value)) {
      assertNoV1SourceProfileFields(childValue as JsonValue);
    }
  }
}

function removedSourceProfileTerms() {
  return [
    "adap" + "ter" + "Key",
    "inven" + "tory",
    "source" + "_specific",
    "Source" + "Specific",
  ];
}
