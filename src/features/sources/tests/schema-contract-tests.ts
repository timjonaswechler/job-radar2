import assert from "node:assert/strict";

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
import type { JsonValue } from "@/lib/api/sources";


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
