import assert from "node:assert/strict";

import { minimalDetailStrategy, minimalDiscoveryStrategy } from "@/features/sources/tests/support/profile-dsl";

import {
  buildCreatedSourceDocument,
  detectedSourceFromProposal,
  emptySourceCreateForm,
  sourceCreateDraftAfterAccessPathChange,
  sourceCreateDraftAfterDetectedSource,
  sourceCreateDraftAfterDetectionResult,
  sourceCreateDraftAfterProfileChange,
  sourceCreateDraftSnapshot,
  sourceCreateFormAfterKeyChange,
  sourceCreateFormAfterNameChange,
  type SourceCreateDraftState,
  isSourceCreateDraftDirty,
} from "@/features/sources/create/source/source-create-model";
import { sourceDetectionOutcomeCopy } from "@/features/sources/create/source/source-detection-panel";
import {
  effectiveSourceConfigSchema,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import type {
  JsonValue,
  RegistrySourceProfile,
  SourceDocument,
  SourceProposal,
} from "@/lib/api/sources";

const greenhouseProfile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/greenhouse.json",
  document: {
    schemaVersion: 3,
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
        discovery: { policy: { type: "first_accepted" }, strategies: [minimalDiscoveryStrategy("jobs_api")] },
        detail: { policy: { type: "first_accepted" }, strategies: [minimalDetailStrategy("detail_api")] },
      },
    ],
  },
};

const leverProfile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/lever.json",
  document: {
    schemaVersion: 3,
    key: "lever",
    name: "Lever",
    kind: "recruiting_system",
    support: { level: "stable" },
    sourceConfigSchema: {
      type: "object",
      required: ["host"],
      properties: {
        host: { type: "string" },
        locale: { type: "string", default: "en-US" },
      },
    },
    accessPaths: [
      {
        key: "posting_api",
        name: "Posting API",
        sourceConfigSchema: {
          type: "object",
          required: ["tenant"],
          properties: { tenant: { type: "string" } },
        },
        discovery: { policy: { type: "first_accepted" }, strategies: [minimalDiscoveryStrategy("posting_api")] },
      },
      {
        key: "xml_feed",
        name: "XML feed",
        sourceConfigSchema: {
          type: "object",
          required: ["feedUrl"],
          properties: { feedUrl: { type: "string" } },
        },
        discovery: { policy: { type: "first_accepted" }, strategies: [minimalDiscoveryStrategy("feed")] },
      },
    ],
  },
};

const profiles = [greenhouseProfile, leverProfile];
const initialDraft: SourceCreateDraftState = {
  form: emptySourceCreateForm,
  keyTouched: false,
  configEntries: [entry("manual", "manualStableAccessKey", "manual-value")],
  directSourceSpecializationText: "",
  jsonPreviewOpen: true,
  saveAttempted: true,
};

const profileSelectionDraft = sourceCreateDraftAfterProfileChange({
  profiles,
  form: initialDraft.form,
  configEntries: initialDraft.configEntries,
  profileKey: "lever",
  createConfigEntryId: stableEntryIds("profile"),
});
assert.equal(profileSelectionDraft.form.profileKey, "lever");
assert.equal(profileSelectionDraft.form.pathKey, "posting_api");
assert.deepEqual(entryValues(profileSelectionDraft.configEntries), [
  ["manualStableAccessKey", "manual-value"],
  ["host", ""],
  ["tenant", ""],
  ["locale", "en-US"],
]);

const accessPathSelectionDraft = sourceCreateDraftAfterAccessPathChange({
  selectedProfile: leverProfile,
  form: profileSelectionDraft.form,
  configEntries: profileSelectionDraft.configEntries,
  pathKey: "xml_feed",
  createConfigEntryId: stableEntryIds("path"),
});
assert.equal(accessPathSelectionDraft.form.profileKey, "lever");
assert.equal(accessPathSelectionDraft.form.pathKey, "xml_feed");
assert.deepEqual(entryValues(accessPathSelectionDraft.configEntries), [
  ["manualStableAccessKey", "manual-value"],
  ["host", ""],
  ["tenant", ""],
  ["locale", "en-US"],
  ["feedUrl", ""],
]);

const proposal: SourceProposal = {
  profileKey: "lever",
  profileName: "Lever",
  recommendedAccessPathKey: "xml_feed",
  recommendedAccessPathName: "XML feed",
  sourceConfig: {
    host: "jobs.lever.co",
    feedUrl: "https://jobs.lever.co/acme.xml",
  },
  keyCandidates: ["acme_jobs"],
  nameCandidates: ["ACME Jobs"],
  captures: { host: "jobs.lever.co" },
  evidence: [{ kind: "url", message: "Matched Lever URL" }],
  supportLevel: "stable",
};

const matchedDraft = sourceCreateDraftAfterDetectionResult({
  draft: initialDraft,
  profiles,
  result: { status: "matched", proposal, diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("matched"),
});
assert.equal(matchedDraft.appliedDetectedSource, true);
assert.deepEqual(matchedDraft.form, {
  name: "ACME Jobs",
  key: "acme_jobs",
  status: "draft",
  profileKey: "lever",
  pathKey: "xml_feed",
});
assert.equal(matchedDraft.keyTouched, false);
assert.equal(matchedDraft.jsonPreviewOpen, false);
assert.equal(matchedDraft.saveAttempted, false);
assert.deepEqual(entryValues(matchedDraft.configEntries), [
  ["host", "jobs.lever.co"],
  ["feedUrl", "https://jobs.lever.co/acme.xml"],
  ["locale", "en-US"],
]);

const ambiguousDraft = sourceCreateDraftAfterDetectionResult({
  draft: initialDraft,
  profiles,
  result: { status: "ambiguous", proposals: [proposal], diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("ambiguous"),
});
assert.deepEqual(ambiguousDraft, {
  ...initialDraft,
  appliedDetectedSource: false,
});

const explicitlyAppliedProposal = sourceCreateDraftAfterDetectedSource({
  profiles,
  detected: detectedSourceFromProposal(proposal)!,
  createConfigEntryId: stableEntryIds("apply"),
});
assert.equal(explicitlyAppliedProposal.form.profileKey, "lever");
assert.equal(explicitlyAppliedProposal.form.pathKey, "xml_feed");
assert.deepEqual(entryValues(explicitlyAppliedProposal.configEntries), [
  ["host", "jobs.lever.co"],
  ["feedUrl", "https://jobs.lever.co/acme.xml"],
  ["locale", "en-US"],
]);

const failedDraft = sourceCreateDraftAfterDetectionResult({
  draft: initialDraft,
  profiles,
  result: { status: "failed", diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("failed"),
});
assert.deepEqual(failedDraft, { ...initialDraft, appliedDetectedSource: false });

const unsupportedDraft = sourceCreateDraftAfterDetectionResult({
  draft: initialDraft,
  profiles,
  result: { status: "unsupported", unsupportedProfiles: [], diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("unsupported"),
});
assert.deepEqual(entryValues(unsupportedDraft.configEntries), [
  ["manualStableAccessKey", "manual-value"],
  ["startUrl", "https://jobs.lever.co/acme"],
]);
const unsupportedWithStartUrl = sourceCreateDraftAfterDetectionResult({
  draft: {
    ...initialDraft,
    configEntries: [entry("start", "startUrl", "https://existing.test/jobs")],
  },
  profiles,
  result: { status: "unsupported", unsupportedProfiles: [], diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("unsupported-existing"),
});
assert.deepEqual(entryValues(unsupportedWithStartUrl.configEntries), [
  ["startUrl", "https://existing.test/jobs"],
]);

const autoKeyForm = sourceCreateFormAfterNameChange(
  emptySourceCreateForm,
  false,
  "ACME Jobs GmbH",
);
assert.equal(autoKeyForm.key, "acme_jobs_gmbh");
const manualKeyForm = sourceCreateFormAfterKeyChange(autoKeyForm, "Custom Key");
assert.equal(manualKeyForm.key, "custom_key");
assert.equal(
  sourceCreateFormAfterNameChange(manualKeyForm, true, "Different Company").key,
  "custom_key",
);

const transitionBuildResult = buildCreatedSourceDocument({
  form: matchedDraft.form,
  configEntries: matchedDraft.configEntries,
  existingSourceKeys: new Set(),
  selectedProfile: leverProfile,
  selectedAccessPath: leverProfile.document.accessPaths[1] ?? null,
  schemaMetadata: sourceConfigSchemaMetadata(
    effectiveSourceConfigSchema(
      leverProfile.document.sourceConfigSchema,
      leverProfile.document.accessPaths[1]?.sourceConfigSchema,
    ),
  ),
});
assert.deepEqual(transitionBuildResult.errors, []);
assert.deepEqual(Object.keys(transitionBuildResult.document?.sourceConfig ?? {}), [
  "host",
  "feedUrl",
  "locale",
]);

const detected = detectedSourceFromProposal({
  profileKey: "greenhouse",
  profileName: "Greenhouse",
  recommendedAccessPathKey: "boards_api",
  recommendedAccessPathName: "Boards API",
  sourceConfig: { boardSlug: "acme" },
  keyCandidates: ["acme"],
  nameCandidates: ["ACME GmbH"],
  captures: { boardSlug: "acme" },
  evidence: [{ kind: "url", message: "Matched board URL" }],
  supportLevel: "stable",
});
assert.deepEqual(detected, {
  profileKey: "greenhouse",
  pathKey: "boards_api",
  key: "acme",
  name: "ACME GmbH",
  sourceConfig: { boardSlug: "acme" },
});

const buildResult = buildCreatedSourceDocument({
  form: {
    name: detected?.name ?? "",
    key: detected?.key ?? "",
    status: "draft",
    profileKey: detected?.profileKey ?? "",
    pathKey: detected?.pathKey ?? "",
  },
  configEntries: [{ id: "boardSlug", key: "boardSlug", value: "acme" }],
  directSourceSpecializationText:
    '[{"key":"boards_api","discovery":{"strategies":[{"key":"jobs_api"}]}}]',
  existingSourceKeys: new Set(),
  selectedProfile: greenhouseProfile,
  selectedAccessPath: greenhouseProfile.document.accessPaths[0] ?? null,
  schemaMetadata: sourceConfigSchemaMetadata(
    greenhouseProfile.document.sourceConfigSchema ?? {},
  ),
});
assert.deepEqual(buildResult.errors, []);
assert.equal(buildResult.document?.schemaVersion, 3);
assert.equal(buildResult.document?.selectedAccessPath.type, "profile_access_path");
assert.deepEqual(buildResult.document?.sourceConfig, { boardSlug: "acme" });
assert.deepEqual(buildResult.document?.accessPaths, [
  {
    key: "boards_api",
    discovery: { strategies: [{ key: "jobs_api" }] },
  },
]);
assertNoV1SourceProfileFields(buildResult.document);

for (const searchRequestCriterion of [
  "keywords",
  "roles",
  "locations",
  "countries",
  "radius",
  "includeRules",
  "excludeRules",
]) {
  assert.equal(
    Object.prototype.hasOwnProperty.call(
      transitionBuildResult.document?.sourceConfig ?? {},
      searchRequestCriterion,
    ),
    false,
  );
}

const cleanCreateDraft = {
  url: "",
  form: emptySourceCreateForm,
  configEntries: [] as SourceConfigEntry[],
  directSourceSpecializationText: "",
};
assert.equal(isSourceCreateDraftDirty(cleanCreateDraft), false);

for (const [field, changedDraft] of [
  ["Detection URL", { ...cleanCreateDraft, url: "https://example.test/jobs" }],
  [
    "name",
    { ...cleanCreateDraft, form: { ...emptySourceCreateForm, name: "ACME" } },
  ],
  ["key", { ...cleanCreateDraft, form: { ...emptySourceCreateForm, key: "acme" } }],
  [
    "status",
    { ...cleanCreateDraft, form: { ...emptySourceCreateForm, status: "active" as const } },
  ],
  [
    "Source Profile",
    {
      ...cleanCreateDraft,
      form: { ...emptySourceCreateForm, profileKey: "greenhouse" },
    },
  ],
  [
    "Selected Access Path",
    {
      ...cleanCreateDraft,
      form: { ...emptySourceCreateForm, pathKey: "boards_api" },
    },
  ],
  [
    "Source Config",
    {
      ...cleanCreateDraft,
      configEntries: [entry("config", "boardSlug", "acme")],
    },
  ],
  [
    "Source specialization",
    { ...cleanCreateDraft, directSourceSpecializationText: "[]" },
  ],
] as const) {
  assert.equal(
    isSourceCreateDraftDirty(changedDraft),
    true,
    `${field} must make a Create draft dirty`,
  );
}

assert.equal(
  isSourceCreateDraftDirty({
    ...cleanCreateDraft,
    form: explicitlyAppliedProposal.form,
    configEntries: explicitlyAppliedProposal.configEntries,
  }),
  true,
  "an applied Source Proposal must make the resulting Create draft dirty",
);
assert.equal(
  isSourceCreateDraftDirty({
    ...cleanCreateDraft,
    configEntries: [
      entry("duplicate-a", "boardSlug", "acme"),
      entry("duplicate-b", "boardSlug", "other"),
    ],
  }),
  true,
  "duplicate raw Source Config rows must stay dirty",
);
assert.equal(
  isSourceCreateDraftDirty({
    ...cleanCreateDraft,
    configEntries: [entry("invalid", "settings", "{invalid")],
    directSourceSpecializationText: "{invalid",
  }),
  true,
  "invalid raw values must stay dirty",
);
assert.deepEqual(
  sourceCreateDraftSnapshot({
    ...cleanCreateDraft,
    configEntries: [
      { id: "first", key: "boardSlug", value: "acme", locked: true },
    ],
  }),
  sourceCreateDraftSnapshot({
    ...cleanCreateDraft,
    configEntries: [
      { id: "second", key: "boardSlug", value: "acme", locked: false },
    ],
  }),
  "Create snapshot must ignore Source Config entry metadata",
);
const createDraftWithUiState: SourceCreateDraftState = {
  form: emptySourceCreateForm,
  keyTouched: true,
  configEntries: [],
  directSourceSpecializationText: "",
  jsonPreviewOpen: true,
  saveAttempted: true,
};
assert.deepEqual(
  sourceCreateDraftSnapshot({ url: "", ...createDraftWithUiState }),
  sourceCreateDraftSnapshot(cleanCreateDraft),
  "Create snapshot must ignore editor and validation UI state",
);
assert.equal(
  isSourceCreateDraftDirty({
    ...cleanCreateDraft,
    url: "https://example.test/jobs",
  }),
  true,
);
assert.equal(
  isSourceCreateDraftDirty(cleanCreateDraft),
  false,
  "fully reverting Create values must restore a clean draft",
);

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

function assertNoV1SourceProfileFields(
  value: JsonValue | SourceDocument | null | undefined,
) {
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

function entry(id: string, key: string, value: string): SourceConfigEntry {
  return { id, key, value };
}

function entryValues(entries: SourceConfigEntry[]) {
  return entries.map((entry) => [entry.key, entry.value]);
}

function stableEntryIds(prefix: string) {
  let nextId = 0;
  return () => `00000000-0000-4000-8000-${prefix}${String(++nextId).padStart(8, "0")}`;
}

function removedSourceProfileTerms() {
  return [
    "adap" + "ter" + "Key",
    "inven" + "tory",
    "source" + "_specific",
    "Source" + "Specific",
  ];
}
