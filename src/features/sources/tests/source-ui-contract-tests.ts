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
  entriesWithSchemaHints,
  sourceConfigFromEntries,
  sourceConfigSchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import {
  activeSchemaVariant,
  createSchemaCatalog,
  schemaConstraints,
  schemaFieldTypeFromSchema,
  schemaForArrayItem,
  schemaForProperty,
  schemaForValue,
  schemaMetadataForObject,
  schemaScalarOptions,
  schemaScalarRules,
} from "@/features/sources/shared/schema-introspection";
import {
  applySchemaGuidedArrayEdit,
  applySchemaGuidedObjectEdit,
  createSchemaGuidedValueEditorModel,
} from "@/features/sources/shared/schema-guided-value-editor";
import { createSchemaValueRows } from "@/features/sources/shared/schema-value-rows";
import {
  profileDslSchemaCatalog,
  profileDslSchemaRefs,
} from "@/features/sources/shared/profile-dsl-schema-catalog";
import {
  buildDiagnosticIndex,
  countOrigins,
  countProfileKinds,
  countSourceStatuses,
  createProfileGridRows,
  createSourceGridRows,
  filterProfileGridRows,
  filterSourceGridRows,
  resolveSource,
  sourceLiveCheckActionsForSource,
  sourceLiveCheckDisplayModel,
} from "@/features/sources/view-model/registry-view-model";
import type {
  CheckReport,
  JsonValue,
  RegistrySource,
  RegistrySourceProfile,
  SourceDocument,
  SourceProposal,
  SourceLiveCheckReportStatus,
  StructuredDiagnostic,
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

const profilesByKey = new Map([[profile.document.key, profile]]);

const pathSourceConfigSchema: JsonValue = {
  type: "object",
  required: ["tenant"],
  properties: {
    tenant: { type: "string", title: "Tenant" },
    pageSize: { type: "integer", default: 50 },
    includeArchived: { type: "boolean", default: false },
    headers: { type: "object" },
    locale: { type: "string", default: "de-DE" },
    optionalNote: { type: "string" },
  },
};
assert.equal(
  effectiveSourceConfigSchema(profile.document.sourceConfigSchema, undefined),
  profile.document.sourceConfigSchema,
);
assert.equal(
  effectiveSourceConfigSchema(undefined, pathSourceConfigSchema),
  pathSourceConfigSchema,
);
assert.deepEqual(effectiveSourceConfigSchema(undefined, undefined), { type: "object" });
const combinedSourceConfigSchema = effectiveSourceConfigSchema(
  profile.document.sourceConfigSchema,
  pathSourceConfigSchema,
);
assert.deepEqual(combinedSourceConfigSchema, {
  allOf: [profile.document.sourceConfigSchema, pathSourceConfigSchema],
});

const combinedSourceConfigMetadata = sourceConfigSchemaMetadata(combinedSourceConfigSchema);
assert.deepEqual([...combinedSourceConfigMetadata.requiredKeys], ["boardSlug", "tenant"]);
assert.deepEqual([...combinedSourceConfigMetadata.properties.keys()], [
  "boardSlug",
  "tenant",
  "pageSize",
  "includeArchived",
  "headers",
  "locale",
  "optionalNote",
]);
assert.equal(combinedSourceConfigMetadata.properties.get("tenant")?.title, "Tenant");

const hintedSourceConfigEntries = entriesWithSchemaHints(
  [entry("existing-board", "boardSlug", "acme")],
  combinedSourceConfigMetadata,
  stableEntryIds("hint"),
);
assert.deepEqual(hintedSourceConfigEntries, [
  entry("existing-board", "boardSlug", "acme"),
  entry("00000000-0000-4000-8000-hint00000001", "tenant", ""),
  entry("00000000-0000-4000-8000-hint00000002", "pageSize", "50"),
  entry("00000000-0000-4000-8000-hint00000003", "includeArchived", "false"),
  entry("00000000-0000-4000-8000-hint00000004", "locale", "de-DE"),
]);

const parsedSourceConfig = sourceConfigFromEntries(
  [
    entry("board", "boardSlug", "acme"),
    entry("tenant", "tenant", "main"),
    entry("page-size", "pageSize", "25"),
    entry("include-archived", "includeArchived", "ja"),
    entry("headers", "headers", '{"Accept":"application/json"}'),
    entry("optional-empty", "optionalNote", ""),
    entry("blank", "", ""),
  ],
  combinedSourceConfigMetadata,
);
assert.deepEqual(parsedSourceConfig, {
  value: {
    boardSlug: "acme",
    tenant: "main",
    pageSize: 25,
    includeArchived: true,
    headers: { Accept: "application/json" },
  },
  errors: [],
});

const duplicateConfigKeys = sourceConfigFromEntries(
  [
    entry("board", "boardSlug", "acme"),
    entry("tenant", "tenant", "main"),
    entry("duplicate", "boardSlug", "other"),
  ],
  combinedSourceConfigMetadata,
);
assert.deepEqual(duplicateConfigKeys.value, { boardSlug: "acme", tenant: "main" });
assert.deepEqual(duplicateConfigKeys.errors, [
  "Der Konfigurations-Key „boardSlug“ ist doppelt vorhanden.",
]);

const invalidJsonConfig = sourceConfigFromEntries(
  [
    entry("board", "boardSlug", "acme"),
    entry("tenant", "tenant", "main"),
    entry("headers", "headers", "{not-json}"),
  ],
  combinedSourceConfigMetadata,
);
assert.deepEqual(invalidJsonConfig.value, { boardSlug: "acme", tenant: "main" });
assert.deepEqual(invalidJsonConfig.errors, ["„headers“ braucht einen gültigen JSON-Wert."]);

const missingRequiredConfig = sourceConfigFromEntries(
  [entry("tenant", "tenant", "main")],
  combinedSourceConfigMetadata,
);
assert.deepEqual(missingRequiredConfig.errors, ["Pflichtwert „boardSlug“ fehlt."]);

// Source Config helpers are frontend preflight/hint logic only. Backend Source/Profile
// Compiler validation stays authoritative for full JSON Schema semantics such as enum,
// pattern, and additionalProperties.
const preflightOnlySchema = sourceConfigSchemaMetadata({
  type: "object",
  required: ["host"],
  additionalProperties: false,
  properties: {
    host: { type: "string", pattern: "^example\\.com$" },
    mode: { type: "string", enum: ["live"] },
  },
});
assert.deepEqual(
  sourceConfigFromEntries(
    [
      entry("host", "host", "not-example.test"),
      entry("mode", "mode", "preview"),
      entry("extra", "extra", "kept-for-compiler-validation"),
    ],
    preflightOnlySchema,
  ),
  {
    value: {
      host: "not-example.test",
      mode: "preview",
      extra: "kept-for-compiler-validation",
    },
    errors: [],
  },
);

const schemaCatalog = createSchemaCatalog([
  {
    $id: "https://job-radar.local/schemas/profile-dsl/common.schema.json",
    $defs: {
      technicalKey: { type: "string", pattern: "^[a-z0-9_]+$" },
      nonEmptyString: { type: "string", minLength: 1 },
    },
  },
  {
    $id: "https://job-radar.local/schemas/profile-dsl/fetch.schema.json",
    $defs: {
      fetch: {
        oneOf: [
          { $ref: "#/$defs/httpFetch" },
          { $ref: "#/$defs/browserFetch" },
        ],
      },
      httpFetch: {
        title: "HTTP fetch",
        type: "object",
        required: ["mode", "url", "timeoutMs"],
        additionalProperties: false,
        properties: {
          mode: { const: "http" },
          method: { type: "string", enum: ["GET", "POST"], default: "GET" },
          url: { $ref: "common.schema.json#/$defs/nonEmptyString" },
          timeoutMs: { type: "integer", minimum: 1 },
        },
      },
      browserFetch: {
        title: "Browser fetch",
        type: "object",
        required: ["mode", "url", "timeoutMs"],
        additionalProperties: false,
        properties: {
          mode: { const: "browser" },
          url: { $ref: "common.schema.json#/$defs/nonEmptyString" },
          timeoutMs: { type: "integer", minimum: 1 },
        },
      },
    },
  },
]);

const sourceConfigWithRefs: JsonValue = {
  type: "object",
  allOf: [
    { $ref: "#/$defs/commonConfig" },
    {
      type: "object",
      required: ["tenant"],
      properties: {
        tenant: { $ref: "#/$defs/tenant" },
        mode: { const: "live" },
      },
    },
  ],
  $defs: {
    commonConfig: {
      type: "object",
      required: ["host"],
      properties: {
        host: { $ref: "#/$defs/tenant" },
      },
    },
    tenant: { type: "string", title: "Tenant value" },
  },
};
const sourceConfigWithRefsMetadata = schemaMetadataForObject(sourceConfigWithRefs);
assert.deepEqual([...sourceConfigWithRefsMetadata.requiredKeys], ["host", "tenant"]);
assert.deepEqual([...sourceConfigWithRefsMetadata.properties.keys()], [
  "host",
  "tenant",
  "mode",
]);
assert.equal(
  sourceConfigWithRefsMetadata.properties.get("host")?.title,
  "Tenant value",
);
assert.equal(schemaFieldTypeFromSchema({ const: "live" }), "string");
assert.deepEqual(schemaScalarOptions({ const: "live" }), [
  { value: "live", label: "live" },
]);

const fetchRef = "profile-dsl/fetch.schema.json#/$defs/fetch";
const httpFetchValue: JsonValue = {
  mode: "http",
  method: "POST",
  url: "{{sourceConfig.host}}",
  timeoutMs: 5000,
};
const activeFetchVariant = activeSchemaVariant(
  { $ref: fetchRef },
  httpFetchValue,
  { catalog: schemaCatalog },
);
assert.equal(activeFetchVariant?.label, "HTTP fetch");
assert.deepEqual(
  schemaScalarOptions(schemaForProperty("method", activeFetchVariant?.schema)),
  [
    { value: "GET", label: "GET" },
    { value: "POST", label: "POST" },
  ],
);
assert.equal(
  schemaFieldTypeFromSchema(schemaForProperty("timeoutMs", activeFetchVariant?.schema)),
  "number",
);

const arrayItemSchema = schemaForArrayItem({
  type: "array",
  items: { $ref: "profile-dsl/fetch.schema.json#/$defs/fetch" },
}, { catalog: schemaCatalog });
assert.equal(
  activeSchemaVariant(
    arrayItemSchema,
    { mode: "browser", url: "/", timeoutMs: 1 },
    { catalog: schemaCatalog },
  )?.label,
  "Browser fetch",
);

const postingDiscoverySchema = profileDslSchemaCatalog.resolveRef(
  profileDslSchemaRefs.postingDiscoveryStep,
);
assert.ok(postingDiscoverySchema);
const postingDiscoverySchemaOptions = {
  catalog: profileDslSchemaCatalog,
  rootSchema: postingDiscoverySchema.rootSchema,
  baseUri: postingDiscoverySchema.baseUri,
};
const strategiesSchema = schemaForProperty(
  "strategies",
  postingDiscoverySchema.schema,
  postingDiscoverySchemaOptions,
);
const postingDiscoveryStrategySchema = schemaForArrayItem(
  strategiesSchema,
  postingDiscoverySchemaOptions,
);
const strategyFetchSchema = schemaForProperty(
  "fetch",
  postingDiscoveryStrategySchema,
  postingDiscoverySchemaOptions,
);
const activeHttpFetchSchema = schemaForValue(
  strategyFetchSchema,
  httpFetchValue,
  postingDiscoverySchemaOptions,
);
assert.deepEqual(
  schemaScalarOptions(
    schemaForProperty("mode", activeHttpFetchSchema, postingDiscoverySchemaOptions),
    postingDiscoverySchemaOptions,
  ),
  [{ value: "http", label: "http" }],
);
assert.deepEqual(
  schemaScalarOptions(
    schemaForProperty("method", activeHttpFetchSchema, postingDiscoverySchemaOptions),
    postingDiscoverySchemaOptions,
  ),
  [
    { value: "GET", label: "GET" },
    { value: "POST", label: "POST" },
  ],
);
assert.equal(
  schemaFieldTypeFromSchema(
    schemaForProperty("timeoutMs", activeHttpFetchSchema, postingDiscoverySchemaOptions),
    postingDiscoverySchemaOptions,
  ),
  "number",
);
assert.deepEqual(
  schemaConstraints(
    schemaForProperty("timeoutMs", activeHttpFetchSchema, postingDiscoverySchemaOptions),
    postingDiscoverySchemaOptions,
  ),
  ["min 1", "max 60000"],
);
assert.deepEqual(
  schemaScalarRules(
    schemaForProperty("mode", activeHttpFetchSchema, postingDiscoverySchemaOptions),
    postingDiscoverySchemaOptions,
  ),
  [{ kind: "const", value: "http", label: "http" }],
);

const postingDiscoveryRows = createSchemaValueRows({
  value: {
    strategies: [
      {
        key: "jobs_api",
        fetch: httpFetchValue,
        parse: { type: "json" },
        select: { type: "json_path", jsonPath: "$.jobs" },
        extract: {
          fields: {
            title: { type: "item_field", key: "title" },
            company: { type: "item_field", key: "company" },
            url: { type: "item_field", key: "url" },
          },
        },
      },
    ],
  },
  schema: postingDiscoverySchema.schema,
  schemaOptions: postingDiscoverySchemaOptions,
  maxDepth: 6,
});
const fetchRow = postingDiscoveryRows.find((row) => row.key === "fetch");
assert.equal(fetchRow?.variantLabel, "HTTP fetch");

const httpFetchRows = createSchemaValueRows({
  value: { ...httpFetchValue, unexpected: true },
  schema: strategyFetchSchema,
  schemaOptions: postingDiscoverySchemaOptions,
  maxDepth: 1,
});
assert.equal(
  httpFetchRows.find((row) => row.key === "unexpected")?.unknown,
  true,
);
assert.equal(httpFetchRows.find((row) => row.key === "mode")?.unknown, false);

const schemaGuidedUnknownKeys = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify({ mode: "http", url: "/jobs", timeoutMs: 1, extra: true }),
  schema: strategyFetchSchema,
  schemaOptions: postingDiscoverySchemaOptions,
  maxDepth: 1,
});
assert.equal(schemaGuidedUnknownKeys.parseState.ok, true);
assert.equal(schemaGuidedUnknownKeys.matchedVariantLabel, "HTTP fetch");
assert.deepEqual(schemaGuidedUnknownKeys.unknownKeyWarnings, [
  { key: "extra", path: "extra" },
]);

const schemaGuidedVariant = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify({ mode: "browser", url: "/jobs", timeoutMs: 1 }),
  schema: strategyFetchSchema,
  schemaOptions: postingDiscoverySchemaOptions,
});
assert.equal(schemaGuidedVariant.matchedVariantLabel, "Browser fetch");
assert.equal(schemaGuidedVariant.unknownKeyWarnings.length, 0);

const invalidSchemaGuidedJson = createSchemaGuidedValueEditorModel({
  rawText: '{"mode":"http",',
  schema: strategyFetchSchema,
  schemaOptions: postingDiscoverySchemaOptions,
});
assert.equal(invalidSchemaGuidedJson.parseState.ok, false);
if (!invalidSchemaGuidedJson.parseState.ok) {
  assert.equal(invalidSchemaGuidedJson.parseState.rawText, '{"mode":"http",');
  assert.ok(invalidSchemaGuidedJson.parseState.error.length > 0);
}
assert.deepEqual(invalidSchemaGuidedJson.unknownKeyWarnings, []);
assert.equal(invalidSchemaGuidedJson.matchedVariantLabel, null);

const schemaGuidedObjectSchema: JsonValue = {
  type: "object",
  required: ["mode"],
  additionalProperties: false,
  properties: {
    mode: { const: "http" },
    method: { type: "string", enum: ["GET", "POST"], default: "GET" },
    timeoutMs: { type: "integer", default: 10 },
    enabled: { type: "boolean", default: true },
    url: { type: "string", format: "uri" },
  },
};
const schemaGuidedEditableObject = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify({ mode: "http", timeoutMs: 5, extra: "warn" }),
  schema: schemaGuidedObjectSchema,
});
assert.deepEqual(
  schemaGuidedEditableObject.editableObjectRows.map((row) => ({
    key: row.key,
    fieldType: row.fieldType,
    required: row.required,
    unknown: row.unknown,
    scalarOptions: row.scalarOptions,
  })),
  [
    {
      key: "mode",
      fieldType: "string",
      required: true,
      unknown: false,
      scalarOptions: [{ value: "http", label: "http" }],
    },
    {
      key: "timeoutMs",
      fieldType: "number",
      required: false,
      unknown: false,
      scalarOptions: [],
    },
    {
      key: "extra",
      fieldType: "string",
      required: false,
      unknown: true,
      scalarOptions: [],
    },
  ],
);
assert.deepEqual(schemaGuidedEditableObject.availableObjectKeys, [
  { key: "method", label: "method", required: false },
  { key: "enabled", label: "enabled", required: false },
  { key: "url", label: "url", required: false },
]);

const addedSchemaGuidedObjectKey = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http" }),
  schema: schemaGuidedObjectSchema,
  edit: { type: "add-property", key: "method" },
});
assert.equal(addedSchemaGuidedObjectKey.ok, true);
if (addedSchemaGuidedObjectKey.ok) {
  assert.deepEqual(JSON.parse(addedSchemaGuidedObjectKey.rawText), {
    mode: "http",
    method: "GET",
  });
}

const editedSchemaGuidedObjectValue = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", timeoutMs: 10, enabled: true }),
  schema: schemaGuidedObjectSchema,
  edit: { type: "set-property-value", key: "timeoutMs", rawValue: "25" },
});
assert.equal(editedSchemaGuidedObjectValue.ok, true);
if (editedSchemaGuidedObjectValue.ok) {
  assert.deepEqual(JSON.parse(editedSchemaGuidedObjectValue.rawText), {
    mode: "http",
    timeoutMs: 25,
    enabled: true,
  });
}
const toggledSchemaGuidedObjectValue = applySchemaGuidedObjectEdit({
  rawText: editedSchemaGuidedObjectValue.ok
    ? editedSchemaGuidedObjectValue.rawText
    : "{}",
  schema: schemaGuidedObjectSchema,
  edit: { type: "set-property-value", key: "enabled", rawValue: "false" },
});
assert.equal(toggledSchemaGuidedObjectValue.ok, true);
if (toggledSchemaGuidedObjectValue.ok) {
  assert.deepEqual(JSON.parse(toggledSchemaGuidedObjectValue.rawText), {
    mode: "http",
    timeoutMs: 25,
    enabled: false,
  });
}

const removedSchemaGuidedObjectKey = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", method: "POST" }),
  schema: schemaGuidedObjectSchema,
  edit: { type: "remove-property", key: "method" },
});
assert.equal(removedSchemaGuidedObjectKey.ok, true);
if (removedSchemaGuidedObjectKey.ok) {
  assert.deepEqual(JSON.parse(removedSchemaGuidedObjectKey.rawText), { mode: "http" });
}
assert.deepEqual(
  applySchemaGuidedObjectEdit({
    rawText: JSON.stringify({ mode: "http" }),
    schema: schemaGuidedObjectSchema,
    edit: { type: "remove-property", key: "mode" },
  }),
  { ok: false, rawText: JSON.stringify({ mode: "http" }), error: "Required key cannot be removed." },
);

const schemaGuidedVariantSchema: JsonValue = {
  oneOf: [
    {
      title: "HTTP fetch",
      type: "object",
      required: ["mode", "url", "timeoutMs"],
      additionalProperties: false,
      properties: {
        mode: { const: "http" },
        method: { type: "string", enum: ["GET", "POST"], default: "GET" },
        url: { type: "string" },
        timeoutMs: { type: "integer", default: 10 },
      },
    },
    {
      title: "Browser fetch",
      type: "object",
      required: ["mode", "url", "timeoutMs"],
      additionalProperties: false,
      properties: {
        mode: { const: "browser" },
        url: { type: "string" },
        timeoutMs: { type: "integer", default: 30 },
        waitUntil: { type: "string", enum: ["networkidle", "domcontentloaded"], default: "networkidle" },
      },
    },
  ],
};
const schemaGuidedVariants = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify({ mode: "http", method: "POST" }),
  schema: schemaGuidedVariantSchema,
});
assert.deepEqual(schemaGuidedVariants.variantOptions, [
  { index: 0, label: "HTTP fetch", active: true },
  { index: 1, label: "Browser fetch", active: false },
]);
assert.equal(schemaGuidedVariants.activeVariantIndex, 0);
const selectedSchemaGuidedVariant = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", method: "POST" }),
  schema: schemaGuidedVariantSchema,
  edit: { type: "select-variant", variantIndex: 1 },
});
assert.equal(selectedSchemaGuidedVariant.ok, true);
if (selectedSchemaGuidedVariant.ok) {
  assert.deepEqual(JSON.parse(selectedSchemaGuidedVariant.rawText), {
    mode: "browser",
    method: "POST",
    url: "",
    timeoutMs: 30,
    waitUntil: "networkidle",
  });
}

const schemaGuidedArraySchema: JsonValue = {
  type: "array",
  items: {
    type: "object",
    required: ["type", "url"],
    additionalProperties: false,
    properties: {
      type: { const: "link" },
      url: { type: "string", default: "https://example.test/jobs" },
      enabled: { type: "boolean", default: true },
    },
  },
};
const schemaGuidedArray = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify([{ type: "link", url: "https://example.test" }]),
  schema: schemaGuidedArraySchema,
});
assert.deepEqual(
  schemaGuidedArray.editableArrayRows.map((row) => ({
    index: row.index,
    key: row.key,
    fieldType: row.fieldType,
    scalarOptions: row.scalarOptions,
  })),
  [{ index: 0, key: "[0]", fieldType: "json", scalarOptions: [] }],
);
const addedSchemaGuidedArrayItem = applySchemaGuidedArrayEdit({
  rawText: "[]",
  schema: schemaGuidedArraySchema,
  edit: { type: "add-item" },
});
assert.equal(addedSchemaGuidedArrayItem.ok, true);
if (addedSchemaGuidedArrayItem.ok) {
  assert.deepEqual(JSON.parse(addedSchemaGuidedArrayItem.rawText), [
    { type: "link", url: "https://example.test/jobs", enabled: true },
  ]);
}
const editedSchemaGuidedArrayItem = applySchemaGuidedArrayEdit({
  rawText: JSON.stringify(["GET"]),
  schema: { type: "array", items: { type: "string", enum: ["GET", "POST"] } },
  edit: { type: "set-item-value", index: 0, rawValue: "POST" },
});
assert.equal(editedSchemaGuidedArrayItem.ok, true);
if (editedSchemaGuidedArrayItem.ok) {
  assert.deepEqual(JSON.parse(editedSchemaGuidedArrayItem.rawText), ["POST"]);
}
const removedSchemaGuidedArrayItem = applySchemaGuidedArrayEdit({
  rawText: JSON.stringify(["GET", "POST"]),
  schema: { type: "array", items: { type: "string" } },
  edit: { type: "remove-item", index: 0 },
});
assert.equal(removedSchemaGuidedArrayItem.ok, true);
if (removedSchemaGuidedArrayItem.ok) {
  assert.deepEqual(JSON.parse(removedSchemaGuidedArrayItem.rawText), ["POST"]);
}
assert.deepEqual(
  applySchemaGuidedObjectEdit({
    rawText: JSON.stringify({ timeoutMs: 10 }),
    schema: {
      type: "object",
      properties: { timeoutMs: { type: "integer" } },
    },
    edit: { type: "set-property-value", key: "timeoutMs", rawValue: "not-a-number" },
  }),
  {
    ok: false,
    rawText: JSON.stringify({ timeoutMs: 10 }),
    error: "„timeoutMs“ must be a number.",
  },
);
assert.deepEqual(
  applySchemaGuidedArrayEdit({
    rawText: JSON.stringify(["GET"]),
    schema: { type: "array", items: { type: "string" } },
    edit: { type: "remove-item", index: 2 },
  }),
  {
    ok: false,
    rawText: JSON.stringify(["GET"]),
    error: "Array index out of range.",
  },
);

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

const sourceRows = createSourceGridRows(
  [source],
  profilesByKey,
  new Map(),
);

assert.equal(sourceRows[0]?.supportLabel, "Stabil");
assert.equal(sourceRows[0]?.validationStateLabel, "Valide");
assert.equal(sourceRows[0]?.capabilitiesSummary, "postingDiscovery, postingDetail");
assert.equal(sourceRows[0]?.profileLabel, "greenhouse / boards_api");
assert.equal(sourceRows[0]?.health, "valid");
for (const removedTerm of removedSourceProfileTerms()) {
  assert.equal((sourceRows[0]?.searchText ?? "").includes(removedTerm), false);
}

const resolution = resolveSource(source, profilesByKey);
assert.equal(resolution.profileAccessPath?.key, "boards_api");
assert.equal(resolution.supportLevel, "stable");
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
const sourceOwnedResolution = resolveSource(sourceOwnedSource, profilesByKey);
assert.equal(sourceOwnedResolution.profile, null);
assert.equal(sourceOwnedResolution.sourceOwnedAccessPath?.key, "html_jobs");
assert.deepEqual(sourceOwnedResolution.effectiveSourceConfigSchema, { type: "object" });
assert.equal(sourceOwnedResolution.supportLevel, "experimental");
assert.deepEqual(sourceOwnedResolution.capabilities, ["postingDiscovery"]);

const missingProfileSource: RegistrySource = {
  origin: "custom",
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
  origin: "custom",
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
const warningProfile: RegistrySourceProfile = {
  origin: "built_in",
  path: "resources/profiles/warning.json",
  document: {
    ...profile.document,
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
    ...profile.document,
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
const missingProfileDiagnostic: StructuredDiagnostic = {
  category: "registry",
  code: "missing_source_profile",
  message: "Selected Source Profile is missing",
  severity: "error",
  path: "/selectedAccessPath/profileKey",
  details: { sourceKey: "missing_profile_source" },
};
const missingAccessPathDiagnostic: StructuredDiagnostic = {
  category: "registry",
  code: "missing_access_path",
  message: "Selected Access Path is missing",
  severity: "error",
  path: "/selectedAccessPath/pathKey",
  details: { sourceKey: "missing_path_source" },
};
const profileWarningDiagnostic: StructuredDiagnostic = {
  category: "registry",
  code: "profile_known_issue",
  message: "Profile has a known issue",
  severity: "warning",
  path: "/support/knownIssues/0",
  details: { sourceProfileKey: "warning_profile" },
};
const unassignedDiagnostic: StructuredDiagnostic = {
  category: "registry",
  code: "unassigned_registry_warning",
  message: "Unassigned registry warning",
  severity: "warning",
  path: "/registry",
  details: { key: "unknown_document" },
};
const registrySources = [
  source,
  sourceOwnedSource,
  missingProfileSource,
  missingAccessPathSource,
];
const registryProfiles = [profile, warningProfile, errorProfile];
const diagnosticIndex = buildDiagnosticIndex(registrySources, registryProfiles, [
  missingProfileDiagnostic,
  missingAccessPathDiagnostic,
  profileWarningDiagnostic,
  unassignedDiagnostic,
]);
assert.deepEqual(
  diagnosticIndex.bySourceKey.get("missing_profile_source")?.map((diagnostic) => diagnostic.code),
  ["missing_source_profile"],
);
assert.deepEqual(
  diagnosticIndex.bySourceKey.get("missing_path_source")?.map((diagnostic) => diagnostic.code),
  ["missing_access_path"],
);
assert.deepEqual(
  diagnosticIndex.byProfileKey.get("warning_profile")?.map((diagnostic) => diagnostic.code),
  ["profile_known_issue"],
);
assert.deepEqual(diagnosticIndex.unassigned.map((diagnostic) => diagnostic.code), [
  "unassigned_registry_warning",
]);

const registrySourceRows = createSourceGridRows(
  registrySources,
  profilesByKey,
  diagnosticIndex.bySourceKey,
);
assert.deepEqual(
  registrySourceRows.map((row) => [
    row.key,
    row.health,
    row.ownDiagnosticsCount,
    row.dependencyDiagnosticsCount,
  ]),
  [
    ["acme", "valid", 0, 0],
    ["one_off", "invalid", 1, 0],
    ["missing_profile_source", "dependency_warning", 0, 1],
    ["missing_path_source", "dependency_warning", 0, 1],
  ],
);
assert.equal(registrySourceRows.find((row) => row.key === "missing_profile_source")?.supportLabel, "—");
assert.equal(
  resolveSource(missingAccessPathSource, profilesByKey).profile?.document.key,
  "greenhouse",
);
assert.equal(resolveSource(missingAccessPathSource, profilesByKey).profileAccessPath, null);
assert.deepEqual(resolveSource(missingAccessPathSource, profilesByKey).capabilities, []);
assert.deepEqual(countSourceStatuses(registrySourceRows), {
  draft: 1,
  active: 2,
  disabled: 1,
});
assert.deepEqual(
  filterSourceGridRows(registrySourceRows, {
    searchQuery: "missing",
    statuses: ["active"],
    origins: [],
    diagnosticsOnly: true,
  }).map((row) => row.key),
  ["missing_profile_source"],
);

const registryProfileRows = createProfileGridRows(
  registryProfiles,
  diagnosticIndex.byProfileKey,
);
assert.deepEqual(
  registryProfileRows.map((row) => [
    row.key,
    row.health,
    row.ownDiagnosticsCount,
    row.dependencyDiagnosticsCount,
  ]),
  [
    ["greenhouse", "valid", 0, 0],
    ["warning_profile", "dependency_warning", 1, 0],
    ["error_profile", "invalid", 1, 0],
  ],
);
assert.deepEqual(countProfileKinds(registryProfileRows), {
  recruiting_system: 1,
  job_portal: 0,
  website_family: 0,
  career_site: 0,
  generic: 2,
});
assert.deepEqual(countOrigins(registryProfileRows), { built_in: 2, custom: 1 });
assert.deepEqual(
  filterProfileGridRows(registryProfileRows, {
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
    ...profile.document,
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
    detect: {
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
assert.equal(
  evidenceRow?.supportEvidenceSummary,
  "Smoke, Manual Review, Schema Check",
);
assert.deepEqual(evidenceRow?.detectionEvidenceLabels, ["URL", "HTTP"]);
assert.equal(
  (evidenceRow?.detectionEvidenceKinds as string[] | undefined)?.includes("url"),
  true,
);
assert.equal(
  (evidenceRow?.supportEvidenceKinds as string[] | undefined)?.includes("url"),
  false,
);

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

const passedLiveCheckModel = sourceLiveCheckDisplayModel(liveCheckStatus({}));
assert.equal(passedLiveCheckModel.displayState, "passed");
assert.equal(passedLiveCheckModel.displayLabel, "Live-Prüfung bestanden");
assert.equal(passedLiveCheckModel.reportStateLabel, "Frisch");
assert.equal(passedLiveCheckModel.reportResultLabel, "Bestanden");
assert.equal(passedLiveCheckModel.diagnostics.length, 0);

const failedLiveCheckModel = sourceLiveCheckDisplayModel(
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
assert.equal(failedLiveCheckModel.displayState, "failed");
assert.equal(failedLiveCheckModel.diagnostics[0]?.code, "source_live_check.activation_blocked");

const staleLiveCheckModel = sourceLiveCheckDisplayModel(
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
assert.equal(staleLiveCheckModel.displayState, "stale");
assert.equal(staleLiveCheckModel.staleFingerprints[0]?.kind, "source_document");
assert.equal(sourceLiveCheckDisplayModel(null).displayState, "unknown");
assert.equal(sourceLiveCheckDisplayModel({ state: "unknown" }).displayLabel, "Unbekannt");
for (const label of [
  passedLiveCheckModel.displayLabel,
  failedLiveCheckModel.displayLabel,
  staleLiveCheckModel.displayLabel,
]) {
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
