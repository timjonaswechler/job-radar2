import assert from "node:assert/strict";

import {
  buildUpdatedSourceDocument,
  isSourceEditDraftDirty,
  sourceEditDraftFromSource,
  sourceEditDraftSnapshot,
} from "@/features/sources/edit/source/source-edit-model";
import { sourceConfigSchemaMetadata } from "@/features/sources/shared/source-config-schema";
import type { RegistrySource } from "@/lib/api/sources";

const source: RegistrySource = {
  origin: "custom",
  path: "sources/acme.json",
  document: {
    schemaVersion: 3,
    key: "acme",
    name: "ACME",
    status: "active",
    sourceConfig: { boardSlug: "acme" },
    accessPaths: [
      {
        key: "boards_api",
        discovery: { strategies: [{ key: "jobs_api" }] },
      },
    ],
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

const schemaMetadata = sourceConfigSchemaMetadata({
  type: "object",
  required: ["boardSlug"],
  properties: { boardSlug: { type: "string" } },
});
const draft = sourceEditDraftFromSource({
  source,
  schemaMetadata,
  createConfigEntryId: stableEntryIds("edit"),
});
assert.equal(draft.name, "ACME");
assert.equal(draft.status, "active");
assert.deepEqual(
  draft.configEntries.map((entry) => [entry.key, entry.value]),
  [["boardSlug", "acme"]],
);
assert.equal(draft.configEntries[0]?.locked, true);
assert.equal(isSourceEditDraftDirty(draft, draft), false);
assert.equal(
  isSourceEditDraftDirty({ ...draft, name: "ACME Updated" }, draft),
  true,
);
assert.equal(
  isSourceEditDraftDirty({ ...draft, status: "disabled" }, draft),
  true,
);
assert.equal(
  isSourceEditDraftDirty(
    {
      ...draft,
      configEntries: [{ id: "changed", key: "boardSlug", value: "other" }],
    },
    draft,
  ),
  true,
);
assert.equal(
  isSourceEditDraftDirty({ ...draft, directSourceSpecializationText: "{invalid" }, draft),
  true,
  "invalid raw Source specialization must stay dirty",
);
assert.equal(
  isSourceEditDraftDirty(
    {
      ...draft,
      configEntries: [
        { id: "a", key: "boardSlug", value: "acme" },
        { id: "b", key: "boardSlug", value: "duplicate" },
      ],
    },
    draft,
  ),
  true,
  "duplicate raw Source Config rows must stay dirty",
);
assert.deepEqual(
  sourceEditDraftSnapshot({
    ...draft,
    configEntries: [
      { id: "new-id", key: "boardSlug", value: "acme", locked: false },
    ],
  }),
  sourceEditDraftSnapshot(draft),
  "Edit snapshot must ignore Source Config entry metadata",
);
const revertedDraft = {
  ...draft,
  name: "Changed and reverted",
  configEntries: [{ id: "temporary", key: "boardSlug", value: "other" }],
};
assert.equal(isSourceEditDraftDirty(revertedDraft, draft), true);
assert.equal(
  isSourceEditDraftDirty(
    {
      ...revertedDraft,
      name: draft.name,
      configEntries: [
        { id: "replacement", key: "boardSlug", value: "acme", locked: false },
      ],
    },
    draft,
  ),
  false,
  "fully reverting Edit values must restore a clean draft",
);
assert.equal(
  draft.directSourceSpecializationText,
  JSON.stringify(source.document.accessPaths, null, 2),
);

const updated = buildUpdatedSourceDocument({
  source,
  name: "ACME Updated",
  status: "disabled",
  configEntries: [
    { id: "board", key: "boardSlug", value: "acme-updated", locked: true },
  ],
  directSourceSpecializationText: "",
  schemaMetadata,
});
assert.deepEqual(updated.errors, []);
assert.equal(updated.document?.key, "acme");
assert.equal(updated.document?.name, "ACME Updated");
assert.equal(updated.document?.status, "disabled");
assert.deepEqual(updated.document?.sourceConfig, { boardSlug: "acme-updated" });
assert.equal(updated.document?.accessPaths, undefined);
assert.deepEqual(updated.document?.selectedAccessPath, source.document.selectedAccessPath);

const invalid = buildUpdatedSourceDocument({
  source,
  name: " ",
  status: "active",
  configEntries: [],
  directSourceSpecializationText: "{}",
  schemaMetadata,
});
assert.equal(invalid.document, null);
assert.deepEqual(invalid.errors, [
  "Name fehlt.",
  "Pflichtwert „boardSlug“ fehlt.",
  "Direkte Source-Spezialisierung muss ein JSON-Array mit Access-Path-Keys sein.",
]);
assert.deepEqual(invalid.configErrors, ["Pflichtwert „boardSlug“ fehlt."]);
assert.deepEqual(invalid.specializationErrors, [
  "Direkte Source-Spezialisierung muss ein JSON-Array mit Access-Path-Keys sein.",
]);

function stableEntryIds(prefix: string) {
  let nextId = 0;
  return () => `00000000-0000-4000-8000-${prefix}${String(++nextId).padStart(8, "0")}`;
}
