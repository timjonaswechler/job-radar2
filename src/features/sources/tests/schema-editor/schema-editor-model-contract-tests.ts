import assert from "node:assert/strict";

import { createSchemaGuidedValueEditorModel } from "@/features/sources/schema-editor/schema-editor-model";
import type { JsonObject } from "@/features/sources/shared/schema-introspection";

const variantSchema: JsonObject = {
  title: "Fetch",
  description: "Fetch configuration",
  oneOf: [
    {
      title: "HTTP fetch",
      type: "object",
      required: ["mode"],
      additionalProperties: false,
      properties: {
        mode: { const: "http" },
        url: { type: "string", format: "uri" },
        timeoutMs: { type: "integer", default: 10 },
        enabled: { type: "boolean", default: true },
      },
    },
    {
      title: "Browser fetch",
      type: "object",
      required: ["mode"],
      additionalProperties: false,
      properties: {
        mode: { const: "browser" },
        url: { type: "string", format: "uri" },
      },
    },
  ],
};

const schemaGuidedUnknownKeys = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify({ mode: "http", url: "/jobs", timeoutMs: 1, extra: true }),
  schema: variantSchema,
  maxDepth: 1,
});
assert.equal(schemaGuidedUnknownKeys.schemaTitle, "Fetch");
assert.equal(schemaGuidedUnknownKeys.schemaDescription, "Fetch configuration");
assert.equal(schemaGuidedUnknownKeys.parseState.ok, true);
assert.equal(schemaGuidedUnknownKeys.matchedVariantLabel, "HTTP fetch");
assert.deepEqual(schemaGuidedUnknownKeys.unknownKeyWarnings, [
  { key: "extra", path: "extra" },
]);
assert.deepEqual(schemaGuidedUnknownKeys.variantOptions, [
  { index: 0, label: "HTTP fetch", active: true },
  { index: 1, label: "Browser fetch", active: false },
]);
assert.equal(schemaGuidedUnknownKeys.activeVariantIndex, 0);

const schemaGuidedVariant = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify({ mode: "browser", url: "/jobs" }),
  schema: variantSchema,
});
assert.equal(schemaGuidedVariant.matchedVariantLabel, "Browser fetch");
assert.equal(schemaGuidedVariant.unknownKeyWarnings.length, 0);

const invalidRawText = '{"mode":"http",';
const invalidSchemaGuidedJson = createSchemaGuidedValueEditorModel({
  rawText: invalidRawText,
  schema: variantSchema,
});
assert.equal(invalidSchemaGuidedJson.parseState.ok, false);
if (!invalidSchemaGuidedJson.parseState.ok) {
  assert.equal(invalidSchemaGuidedJson.parseState.rawText, invalidRawText);
  assert.ok(invalidSchemaGuidedJson.parseState.error.length > 0);
}
assert.deepEqual(invalidSchemaGuidedJson.unknownKeyWarnings, []);
assert.equal(invalidSchemaGuidedJson.matchedVariantLabel, null);
assert.deepEqual(invalidSchemaGuidedJson.editableObjectRows, []);
assert.deepEqual(invalidSchemaGuidedJson.editableArrayRows, []);

const editableObjectSchema: JsonObject = {
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
  schema: editableObjectSchema,
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

const arraySchema: JsonObject = {
  type: "array",
  items: {
    type: "object",
    required: ["type", "url"],
    additionalProperties: false,
    properties: {
      type: { const: "link" },
      url: { type: "string" },
    },
  },
};
const schemaGuidedArray = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify([{ type: "link", url: "https://example.test" }]),
  schema: arraySchema,
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

const rootSchema: JsonObject = {
  $id: "https://example.test/root.json",
  $defs: {
    closedObject: {
      type: "object",
      additionalProperties: false,
      properties: { known: { type: "string" } },
    },
  },
};
const referencedModel = createSchemaGuidedValueEditorModel({
  rawText: JSON.stringify({ known: "yes", extra: "warn" }),
  schema: { $ref: "#/$defs/closedObject" },
  schemaOptions: {
    rootSchema,
    baseUri: "https://example.test/root.json",
  },
});
assert.deepEqual(referencedModel.unknownKeyWarnings, [
  { key: "extra", path: "extra" },
]);
