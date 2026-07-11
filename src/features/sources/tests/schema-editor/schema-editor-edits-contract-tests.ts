import assert from "node:assert/strict";

import {
  applySchemaGuidedArrayEdit,
  applySchemaGuidedObjectEdit,
} from "@/features/sources/schema-editor/schema-editor-edits";
import type { JsonObject } from "@/features/sources/shared/schema-introspection";

const objectSchema: JsonObject = {
  type: "object",
  required: ["mode"],
  additionalProperties: false,
  properties: {
    mode: { const: "http" },
    method: { type: "string", enum: ["GET", "POST"], default: "GET" },
    timeoutMs: { type: "integer", default: 10 },
    enabled: { type: "boolean", default: true },
    headers: { type: "object" },
  },
};

const addedObjectKey = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http" }),
  schema: objectSchema,
  edit: { type: "add-property", key: "method" },
});
assert.equal(addedObjectKey.ok, true);
if (addedObjectKey.ok) {
  assert.equal(addedObjectKey.rawText, '{\n  "mode": "http",\n  "method": "GET"\n}');
}

const addedFreeObjectKey = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http" }),
  schema: objectSchema,
  edit: { type: "add-property", key: " custom " },
});
assert.equal(addedFreeObjectKey.ok, true);
if (addedFreeObjectKey.ok) {
  assert.deepEqual(JSON.parse(addedFreeObjectKey.rawText), {
    mode: "http",
    custom: "",
  });
}

const editedNumberValue = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", timeoutMs: 10, enabled: true }),
  schema: objectSchema,
  edit: { type: "set-property-value", key: "timeoutMs", rawValue: "25" },
});
assert.equal(editedNumberValue.ok, true);
if (editedNumberValue.ok) {
  assert.deepEqual(JSON.parse(editedNumberValue.rawText), {
    mode: "http",
    timeoutMs: 25,
    enabled: true,
  });
}

const toggledBooleanValue = applySchemaGuidedObjectEdit({
  rawText: editedNumberValue.ok ? editedNumberValue.rawText : "{}",
  schema: objectSchema,
  edit: { type: "set-property-value", key: "enabled", rawValue: "false" },
});
assert.equal(toggledBooleanValue.ok, true);
if (toggledBooleanValue.ok) {
  assert.deepEqual(JSON.parse(toggledBooleanValue.rawText), {
    mode: "http",
    timeoutMs: 25,
    enabled: false,
  });
}

const editedEnumValue = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", method: "GET" }),
  schema: objectSchema,
  edit: { type: "set-property-value", key: "method", rawValue: "POST" },
});
assert.equal(editedEnumValue.ok, true);
if (editedEnumValue.ok) {
  assert.deepEqual(JSON.parse(editedEnumValue.rawText), {
    mode: "http",
    method: "POST",
  });
}

const editedJsonValue = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", headers: {} }),
  schema: objectSchema,
  edit: {
    type: "set-property-value",
    key: "headers",
    rawValue: '{"Accept":"application/json"}',
  },
});
assert.equal(editedJsonValue.ok, true);
if (editedJsonValue.ok) {
  assert.deepEqual(JSON.parse(editedJsonValue.rawText), {
    mode: "http",
    headers: { Accept: "application/json" },
  });
}

const removedObjectKey = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", method: "POST" }),
  schema: objectSchema,
  edit: { type: "remove-property", key: "method" },
});
assert.equal(removedObjectKey.ok, true);
if (removedObjectKey.ok) {
  assert.deepEqual(JSON.parse(removedObjectKey.rawText), { mode: "http" });
}
assert.deepEqual(
  applySchemaGuidedObjectEdit({
    rawText: JSON.stringify({ mode: "http" }),
    schema: objectSchema,
    edit: { type: "remove-property", key: "mode" },
  }),
  {
    ok: false,
    rawText: JSON.stringify({ mode: "http" }),
    error: "Required key cannot be removed.",
  },
);
assert.deepEqual(
  applySchemaGuidedObjectEdit({
    rawText: JSON.stringify({ mode: "http" }),
    schema: objectSchema,
    edit: { type: "add-property", key: " mode " },
  }),
  {
    ok: false,
    rawText: JSON.stringify({ mode: "http" }),
    error: "Key already exists.",
  },
);

const variantSchema: JsonObject = {
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
        waitUntil: {
          type: "string",
          enum: ["networkidle", "domcontentloaded"],
          default: "networkidle",
        },
      },
    },
  ],
};
const selectedVariant = applySchemaGuidedObjectEdit({
  rawText: JSON.stringify({ mode: "http", method: "POST" }),
  schema: variantSchema,
  edit: { type: "select-variant", variantIndex: 1 },
});
assert.equal(selectedVariant.ok, true);
if (selectedVariant.ok) {
  assert.deepEqual(JSON.parse(selectedVariant.rawText), {
    mode: "browser",
    method: "POST",
    url: "",
    timeoutMs: 30,
    waitUntil: "networkidle",
  });
}

const arraySchema: JsonObject = {
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
const addedArrayItem = applySchemaGuidedArrayEdit({
  rawText: "[]",
  schema: arraySchema,
  edit: { type: "add-item" },
});
assert.equal(addedArrayItem.ok, true);
if (addedArrayItem.ok) {
  assert.deepEqual(JSON.parse(addedArrayItem.rawText), [
    { type: "link", url: "https://example.test/jobs", enabled: true },
  ]);
}
const editedArrayItem = applySchemaGuidedArrayEdit({
  rawText: JSON.stringify(["GET"]),
  schema: { type: "array", items: { type: "string", enum: ["GET", "POST"] } },
  edit: { type: "set-item-value", index: 0, rawValue: "POST" },
});
assert.equal(editedArrayItem.ok, true);
if (editedArrayItem.ok) {
  assert.deepEqual(JSON.parse(editedArrayItem.rawText), ["POST"]);
}
const removedArrayItem = applySchemaGuidedArrayEdit({
  rawText: JSON.stringify(["GET", "POST"]),
  schema: { type: "array", items: { type: "string" } },
  edit: { type: "remove-item", index: 0 },
});
assert.equal(removedArrayItem.ok, true);
if (removedArrayItem.ok) {
  assert.deepEqual(JSON.parse(removedArrayItem.rawText), ["POST"]);
}

assert.deepEqual(
  applySchemaGuidedObjectEdit({
    rawText: JSON.stringify({ timeoutMs: 10 }),
    schema: {
      type: "object",
      properties: { timeoutMs: { type: "integer" } },
    },
    edit: {
      type: "set-property-value",
      key: "timeoutMs",
      rawValue: "not-a-number",
    },
  }),
  {
    ok: false,
    rawText: JSON.stringify({ timeoutMs: 10 }),
    error: "„timeoutMs“ must be a number.",
  },
);
assert.deepEqual(
  applySchemaGuidedObjectEdit({
    rawText: JSON.stringify({ headers: {} }),
    schema: {
      type: "object",
      properties: { headers: { type: "object" } },
    },
    edit: { type: "set-property-value", key: "headers", rawValue: "{" },
  }),
  {
    ok: false,
    rawText: JSON.stringify({ headers: {} }),
    error: "„headers“ must be valid JSON.",
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
assert.deepEqual(
  applySchemaGuidedObjectEdit({
    rawText: "not json",
    schema: objectSchema,
    edit: { type: "add-property", key: "method" },
  }).rawText,
  "not json",
);
