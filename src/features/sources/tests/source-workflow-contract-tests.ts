import assert from "node:assert/strict";

import { sourceDetectionOutcomeCopy } from "@/features/sources/add/source/source-detection-panel";
import {
  buildSourceDocument,
  detectedSourceFromProposal,
  emptySourceForm,
  sourceAddDraftAfterAccessPathChange,
  sourceAddDraftAfterDetectionResult,
  sourceAddDraftAfterDetectedSource,
  sourceAddDraftAfterProfileChange,
  sourceOverridesFromText,
  sourceOverridesStarterForAccessPath,
  sourceFormAfterKeyChange,
  sourceFormAfterNameChange,
  type SourceAddDraftState,
} from "@/features/sources/add/source/source-add-model";
import {
  buildUpdatedSourceDocument,
  sourceEditDraftFromSource,
} from "@/features/sources/edit/source/source-edit-model";
import {
  effectiveSourceConfigSchema,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import {
  profileDslSchemaCatalog,
  profileDslSchemaRefs,
} from "@/features/sources/shared/profile-dsl-schema-catalog";
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

const sourceAddTransitionProfile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/lever.json",
  document: {
    schemaVersion: 2,
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
        postingDiscovery: { strategies: [{ key: "posting_api" }] },
      },
      {
        key: "xml_feed",
        name: "XML feed",
        sourceConfigSchema: {
          type: "object",
          required: ["feedUrl"],
          properties: { feedUrl: { type: "string" } },
        },
        postingDiscovery: { strategies: [{ key: "feed" }] },
      },
    ],
  },
};
const transitionProfiles = [profile, sourceAddTransitionProfile];
const sourceAddInitialDraft: SourceAddDraftState = {
  form: emptySourceForm,
  keyTouched: false,
  configEntries: [entry("manual", "manualStableAccessKey", "manual-value")],
  sourceOverridesText: "",
  jsonPreviewOpen: true,
  saveAttempted: true,
};

const profileSelectionDraft = sourceAddDraftAfterProfileChange({
  profiles: transitionProfiles,
  form: sourceAddInitialDraft.form,
  configEntries: sourceAddInitialDraft.configEntries,
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

const sourceOverridesParseResult = sourceOverridesFromText(
  '{"strategyOverrides":[{"step":"postingDiscovery","strategyKey":"posting_api"}]}',
);
assert.deepEqual(sourceOverridesParseResult.errors, []);
assert.deepEqual(sourceOverridesParseResult.value, {
  strategyOverrides: [{ step: "postingDiscovery", strategyKey: "posting_api" }],
});
assert.deepEqual(sourceOverridesFromText("   "), { value: null, errors: [] });
assert.deepEqual(sourceOverridesFromText("[]").errors, [
  "Source Overrides müssen ein JSON-Objekt sein.",
]);
assert.deepEqual(sourceOverridesFromText("{not-json}").errors, [
  "Source Overrides brauchen gültiges JSON.",
]);
const sourceOverridesSchema = profileDslSchemaCatalog.resolveRef(
  profileDslSchemaRefs.sourceOverrides,
);
assert.equal(sourceOverridesSchema?.schema.type, "object");
assert.ok(sourceOverridesSchema?.schema.properties);
assert.equal(
  sourceOverridesStarterForAccessPath(sourceAddTransitionProfile.document.accessPaths[0] ?? null),
  JSON.stringify(
    {
      strategyOverrides: [
        { step: "postingDiscovery", strategyKey: "posting_api" },
      ],
    },
    null,
    2,
  ),
);
assert.equal(
  sourceOverridesStarterForAccessPath(null),
  JSON.stringify(
    {
      strategyOverrides: [{ step: "postingDiscovery", strategyKey: "" }],
    },
    null,
    2,
  ),
);

const accessPathSelectionDraft = sourceAddDraftAfterAccessPathChange({
  selectedProfile: sourceAddTransitionProfile,
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

const detectedTransitionProposal: SourceProposal = {
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
const matchedDetectionDraft = sourceAddDraftAfterDetectionResult({
  draft: sourceAddInitialDraft,
  profiles: transitionProfiles,
  result: {
    status: "matched",
    proposal: detectedTransitionProposal,
    diagnostics: [],
  },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("matched"),
});
assert.equal(matchedDetectionDraft.appliedDetectedSource, true);
assert.deepEqual(matchedDetectionDraft.form, {
  name: "ACME Jobs",
  key: "acme_jobs",
  status: "draft",
  profileKey: "lever",
  pathKey: "xml_feed",
});
assert.equal(matchedDetectionDraft.keyTouched, false);
assert.equal(matchedDetectionDraft.jsonPreviewOpen, false);
assert.equal(matchedDetectionDraft.saveAttempted, false);
assert.deepEqual(entryValues(matchedDetectionDraft.configEntries), [
  ["host", "jobs.lever.co"],
  ["feedUrl", "https://jobs.lever.co/acme.xml"],
  ["locale", "en-US"],
]);

const ambiguousDetectionDraft = sourceAddDraftAfterDetectionResult({
  draft: sourceAddInitialDraft,
  profiles: transitionProfiles,
  result: {
    status: "ambiguous",
    proposals: [detectedTransitionProposal],
    diagnostics: [],
  },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("ambiguous"),
});
assert.deepEqual(ambiguousDetectionDraft, {
  ...sourceAddInitialDraft,
  appliedDetectedSource: false,
});
const explicitlyAppliedAmbiguousProposal = sourceAddDraftAfterDetectedSource({
  profiles: transitionProfiles,
  detected: detectedSourceFromProposal(detectedTransitionProposal)!,
  createConfigEntryId: stableEntryIds("apply"),
});
assert.equal(explicitlyAppliedAmbiguousProposal.form.profileKey, "lever");
assert.equal(explicitlyAppliedAmbiguousProposal.form.pathKey, "xml_feed");
assert.deepEqual(entryValues(explicitlyAppliedAmbiguousProposal.configEntries), [
  ["host", "jobs.lever.co"],
  ["feedUrl", "https://jobs.lever.co/acme.xml"],
  ["locale", "en-US"],
]);

const failedDetectionDraft = sourceAddDraftAfterDetectionResult({
  draft: sourceAddInitialDraft,
  profiles: transitionProfiles,
  result: { status: "failed", diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("failed"),
});
assert.deepEqual(failedDetectionDraft, {
  ...sourceAddInitialDraft,
  appliedDetectedSource: false,
});

const unsupportedDetectionDraft = sourceAddDraftAfterDetectionResult({
  draft: sourceAddInitialDraft,
  profiles: transitionProfiles,
  result: { status: "unsupported", unsupportedProfiles: [], diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("unsupported"),
});
assert.deepEqual(entryValues(unsupportedDetectionDraft.configEntries), [
  ["manualStableAccessKey", "manual-value"],
  ["startUrl", "https://jobs.lever.co/acme"],
]);
const unsupportedWithStartUrlDraft = sourceAddDraftAfterDetectionResult({
  draft: {
    ...sourceAddInitialDraft,
    configEntries: [entry("start", "startUrl", "https://existing.test/jobs")],
  },
  profiles: transitionProfiles,
  result: { status: "unsupported", unsupportedProfiles: [], diagnostics: [] },
  trimmedUrl: "https://jobs.lever.co/acme",
  createConfigEntryId: stableEntryIds("unsupported-existing"),
});
assert.deepEqual(entryValues(unsupportedWithStartUrlDraft.configEntries), [
  ["startUrl", "https://existing.test/jobs"],
]);

const autoKeyForm = sourceFormAfterNameChange(
  emptySourceForm,
  false,
  "ACME Jobs GmbH",
);
assert.equal(autoKeyForm.key, "acme_jobs_gmbh");
const manualKeyForm = sourceFormAfterKeyChange(autoKeyForm, "Custom Key");
assert.equal(manualKeyForm.key, "custom_key");
assert.equal(
  sourceFormAfterNameChange(manualKeyForm, true, "Different Company").key,
  "custom_key",
);

const transitionBuildResult = buildSourceDocument({
  form: matchedDetectionDraft.form,
  configEntries: matchedDetectionDraft.configEntries,
  existingSourceKeys: new Set(),
  selectedProfile: sourceAddTransitionProfile,
  selectedAccessPath: sourceAddTransitionProfile.document.accessPaths[1] ?? null,
  schemaMetadata: sourceConfigSchemaMetadata(
    effectiveSourceConfigSchema(
      sourceAddTransitionProfile.document.sourceConfigSchema,
      sourceAddTransitionProfile.document.accessPaths[1]?.sourceConfigSchema,
    ),
  ),
});
assert.deepEqual(transitionBuildResult.errors, []);
assert.deepEqual(Object.keys(transitionBuildResult.document?.sourceConfig ?? {}), [
  "host",
  "feedUrl",
  "locale",
]);

const editableRegistrySource: RegistrySource = {
  ...source,
  document: {
    ...source.document,
    sourceOverrides: {
      strategyOverrides: [{ step: "postingDiscovery", strategyKey: "jobs_api" }],
    },
  },
};
const editSchemaMetadata = sourceConfigSchemaMetadata(profile.document.sourceConfigSchema ?? {});
const editDraft = sourceEditDraftFromSource({
  source: editableRegistrySource,
  schemaMetadata: editSchemaMetadata,
  createConfigEntryId: stableEntryIds("edit"),
});
assert.equal(editDraft.name, "ACME");
assert.equal(editDraft.status, "active");
assert.deepEqual(entryValues(editDraft.configEntries), [["boardSlug", "acme"]]);
assert.equal(editDraft.configEntries[0]?.locked, true);
assert.equal(
  editDraft.sourceOverridesText,
  JSON.stringify(editableRegistrySource.document.sourceOverrides, null, 2),
);
const updatedSourceDocument = buildUpdatedSourceDocument({
  source: editableRegistrySource,
  name: "ACME Updated",
  status: "disabled",
  configEntries: [entry("board", "boardSlug", "acme-updated")],
  sourceOverridesText: "",
  schemaMetadata: editSchemaMetadata,
});
assert.deepEqual(updatedSourceDocument.errors, []);
assert.equal(updatedSourceDocument.document?.key, "acme");
assert.equal(updatedSourceDocument.document?.name, "ACME Updated");
assert.equal(updatedSourceDocument.document?.status, "disabled");
assert.deepEqual(updatedSourceDocument.document?.sourceConfig, {
  boardSlug: "acme-updated",
});
assert.equal(updatedSourceDocument.document?.sourceOverrides, undefined);

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
  supportLevel: "stable",
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
  sourceOverridesText:
    '{"strategyOverrides":[{"step":"postingDiscovery","strategyKey":"jobs_api"}]}',
  existingSourceKeys: new Set(),
  selectedProfile: profile,
  selectedAccessPath: profile.document.accessPaths[0] ?? null,
  schemaMetadata: sourceConfigSchemaMetadata(profile.document.sourceConfigSchema ?? {}),
});

assert.deepEqual(buildResult.errors, []);
assert.equal(buildResult.document?.schemaVersion, 2);
assert.equal(buildResult.document?.selectedAccessPath.type, "profile_access_path");
assert.deepEqual(buildResult.document?.sourceConfig, { boardSlug: "acme" });
assert.deepEqual(buildResult.document?.sourceOverrides, {
  strategyOverrides: [{ step: "postingDiscovery", strategyKey: "jobs_api" }],
});
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
