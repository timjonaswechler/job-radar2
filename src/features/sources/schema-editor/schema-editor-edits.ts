import {
  isJsonObject,
  schemaDefaultValue,
  schemaFieldTypeFromSchema,
  schemaForArrayItem,
  schemaForProperty,
  schemaForValue,
  schemaMetadataForObject,
  schemaScalarOptions,
  type JsonObject,
  type SchemaResolutionOptions,
} from "@/features/sources/shared/schema-introspection";
import {
  editorSchemaOptions,
  schemaGuidedVariants,
} from "@/features/sources/schema-editor/schema-editor-model";
import type { JsonValue } from "@/lib/api/sources";

export type SchemaGuidedObjectEdit =
  | { type: "add-property"; key: string }
  | { type: "remove-property"; key: string }
  | { type: "set-property-value"; key: string; rawValue: string }
  | { type: "select-variant"; variantIndex: number };

export type SchemaGuidedArrayEdit =
  | { type: "add-item" }
  | { type: "remove-item"; index: number }
  | { type: "set-item-value"; index: number; rawValue: string };

export type SchemaGuidedEditResult =
  | { ok: true; rawText: string }
  | { ok: false; rawText: string; error: string };

export type SchemaGuidedObjectEditResult = SchemaGuidedEditResult;
export type SchemaGuidedArrayEditResult = SchemaGuidedEditResult;

export function applySchemaGuidedObjectEdit({
  rawText,
  schema,
  schemaOptions,
  edit,
}: {
  rawText: string;
  schema?: JsonObject;
  schemaOptions?: SchemaResolutionOptions;
  edit: SchemaGuidedObjectEdit;
}): SchemaGuidedObjectEditResult {
  const options = editorSchemaOptions(schema, schemaOptions);
  const parseState = parseJsonText(rawText);
  if (!parseState.ok) return { ok: false, rawText, error: parseState.error };
  if (!isJsonObject(parseState.value)) {
    return { ok: false, rawText, error: "JSON value must be an object." };
  }

  const valueSchema = schemaForValue(schema, parseState.value, options);
  const metadata = schemaMetadataForObject(valueSchema, options);
  const nextValue: JsonObject = { ...parseState.value };

  if (edit.type === "select-variant") {
    const selectedVariant = schemaGuidedVariants(schema, options).find(
      (variant) => variant.index === edit.variantIndex,
    );
    if (!selectedVariant) {
      return { ok: false, rawText, error: "Schema variant not found." };
    }
    seedObjectForVariant(nextValue, selectedVariant.schema, options);
    return { ok: true, rawText: stringifyJson(nextValue) };
  }

  if (edit.type === "remove-property") {
    if (metadata.requiredKeys.has(edit.key)) {
      return { ok: false, rawText, error: "Required key cannot be removed." };
    }
    delete nextValue[edit.key];
    return { ok: true, rawText: stringifyJson(nextValue) };
  }

  const key = edit.key.trim();
  if (!key) return { ok: false, rawText, error: "Key is required." };

  if (edit.type === "add-property") {
    if (Object.prototype.hasOwnProperty.call(nextValue, key)) {
      return { ok: false, rawText, error: "Key already exists." };
    }
    const propertySchema = schemaForProperty(key, valueSchema, options);
    nextValue[key] = defaultValueForSchema(propertySchema, options);
    return { ok: true, rawText: stringifyJson(nextValue) };
  }

  const propertySchema = schemaForProperty(key, valueSchema, options);
  const convertedValue = valueFromRawInput({
    key,
    rawValue: edit.rawValue,
    schema: propertySchema,
    schemaOptions: options,
  });
  if (!convertedValue.ok) {
    return { ok: false, rawText, error: convertedValue.error };
  }

  nextValue[key] = convertedValue.value;
  return { ok: true, rawText: stringifyJson(nextValue) };
}

export function applySchemaGuidedArrayEdit({
  rawText,
  schema,
  schemaOptions,
  edit,
}: {
  rawText: string;
  schema?: JsonObject;
  schemaOptions?: SchemaResolutionOptions;
  edit: SchemaGuidedArrayEdit;
}): SchemaGuidedArrayEditResult {
  const options = editorSchemaOptions(schema, schemaOptions);
  const parseState = parseJsonText(rawText);
  if (!parseState.ok) return { ok: false, rawText, error: parseState.error };
  if (!Array.isArray(parseState.value)) {
    return { ok: false, rawText, error: "JSON value must be an array." };
  }

  const valueSchema = schemaForValue(schema, parseState.value, options);
  const itemSchema = schemaForArrayItem(valueSchema, options);
  const nextValue = [...parseState.value];

  if (edit.type === "add-item") {
    nextValue.push(defaultValueForSchema(itemSchema, options));
    return { ok: true, rawText: stringifyJson(nextValue) };
  }

  if (edit.index < 0 || edit.index >= nextValue.length) {
    return { ok: false, rawText, error: "Array index out of range." };
  }

  if (edit.type === "remove-item") {
    nextValue.splice(edit.index, 1);
    return { ok: true, rawText: stringifyJson(nextValue) };
  }

  const resolvedItemSchema = schemaForValue(
    itemSchema,
    nextValue[edit.index],
    options,
  );
  const convertedValue = valueFromRawInput({
    key: `[${edit.index}]`,
    rawValue: edit.rawValue,
    schema: resolvedItemSchema,
    schemaOptions: options,
  });
  if (!convertedValue.ok) {
    return { ok: false, rawText, error: convertedValue.error };
  }

  nextValue[edit.index] = convertedValue.value;
  return { ok: true, rawText: stringifyJson(nextValue) };
}

function seedObjectForVariant(
  value: JsonObject,
  variantSchema: JsonObject,
  schemaOptions: SchemaResolutionOptions,
) {
  const metadata = schemaMetadataForObject(variantSchema, schemaOptions);
  for (const [key, propertySchema] of metadata.properties) {
    const constValue = schemaConstValue(propertySchema);
    if (constValue !== undefined) {
      value[key] = constValue;
      continue;
    }

    if (
      !Object.prototype.hasOwnProperty.call(value, key) &&
      (metadata.requiredKeys.has(key) ||
        schemaDefaultValue(propertySchema, schemaOptions) !== undefined)
    ) {
      value[key] = defaultValueForSchema(propertySchema, schemaOptions);
    }
  }
}

function schemaConstValue(
  schema: JsonObject | undefined,
): string | number | boolean | undefined {
  if (!schema || !("const" in schema)) return undefined;
  const value = schema.const;
  if (
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  ) {
    return value;
  }
  return undefined;
}

function valueFromRawInput({
  key,
  rawValue,
  schema,
  schemaOptions,
}: {
  key: string;
  rawValue: string;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions;
}): { ok: true; value: JsonValue } | { ok: false; error: string } {
  const scalarOption = schemaScalarOptions(schema, schemaOptions).find(
    (option) => jsonValueToInputValue(option.value) === rawValue,
  );
  if (scalarOption) return { ok: true, value: scalarOption.value };

  const fieldType = schemaFieldTypeFromSchema(schema, schemaOptions);
  if (fieldType === "number") {
    const value = Number(rawValue);
    if (!Number.isFinite(value)) {
      return { ok: false, error: `„${key}“ must be a number.` };
    }
    return { ok: true, value };
  }

  if (fieldType === "boolean") {
    const normalized = rawValue.trim().toLocaleLowerCase("de");
    if (["true", "1", "ja", "yes"].includes(normalized)) {
      return { ok: true, value: true };
    }
    if (["false", "0", "nein", "no"].includes(normalized)) {
      return { ok: true, value: false };
    }
    return { ok: false, error: `„${key}“ must be true or false.` };
  }

  if (fieldType === "json") {
    try {
      return { ok: true, value: JSON.parse(rawValue) as JsonValue };
    } catch {
      return { ok: false, error: `„${key}“ must be valid JSON.` };
    }
  }

  return { ok: true, value: rawValue };
}

function defaultValueForSchema(
  schema: JsonObject | undefined,
  schemaOptions: SchemaResolutionOptions,
): JsonValue {
  const defaultValue = schemaDefaultValue(schema, schemaOptions);
  if (defaultValue !== undefined) return defaultValue;

  const scalarOption = schemaScalarOptions(schema, schemaOptions)[0];
  if (scalarOption) return scalarOption.value;

  const fieldType = schemaFieldTypeFromSchema(schema, schemaOptions);
  if (fieldType === "number") return 0;
  if (fieldType === "boolean") return false;
  if (fieldType === "json") {
    if (schema?.type === "array") return [];
    const value: JsonObject = {};
    if (schema) seedObjectForVariant(value, schema, schemaOptions);
    return value;
  }
  return "";
}

function jsonValueToInputValue(value: JsonValue | undefined): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return JSON.stringify(value);
}

function stringifyJson(value: JsonValue) {
  return JSON.stringify(value, null, 2);
}

function parseJsonText(
  rawText: string,
):
  | { ok: true; value: JsonValue }
  | { ok: false; rawText: string; error: string } {
  try {
    return { ok: true, value: JSON.parse(rawText) as JsonValue };
  } catch (error) {
    return {
      ok: false,
      rawText,
      error: error instanceof Error ? error.message : "Ungültiger JSON-Wert.",
    };
  }
}
