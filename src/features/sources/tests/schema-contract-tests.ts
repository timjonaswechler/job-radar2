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
