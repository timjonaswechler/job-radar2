import {
  activeSchemaVariant,
  isJsonObject,
  resolveSchema,
  schemaConstraints,
  schemaFieldTypeFromSchema,
  schemaForArrayItem,
  schemaForProperty,
  schemaForValue,
  schemaMetadataForObject,
  schemaScalarOptions,
  type JsonObject,
  type SchemaFieldType,
  type SchemaResolutionOptions,
  type SchemaScalarOption,
} from "@/features/sources/shared/schema-introspection";
import {
  createSchemaValueRows,
  type SchemaValueTreeRowModel,
} from "@/features/sources/shared/schema-value-rows";
import type { JsonValue } from "@/lib/api/sources";

export type SchemaGuidedValueEditorModel = {
  rawText: string;
  schemaTitle: string | null;
  schemaDescription: string | null;
  parseState:
    | { ok: true; value: JsonValue }
    | { ok: false; rawText: string; error: string };
  matchedVariantLabel: string | null;
  unknownKeyWarnings: SchemaGuidedUnknownKeyWarning[];
  rows: SchemaValueTreeRowModel[];
  editableObjectRows: SchemaGuidedEditableObjectRow[];
  editableArrayRows: SchemaGuidedEditableArrayRow[];
  availableObjectKeys: SchemaGuidedObjectKeyOption[];
  variantOptions: SchemaGuidedVariantOption[];
  activeVariantIndex: number | null;
};

export type SchemaGuidedUnknownKeyWarning = {
  key: string;
  path: string;
};

export type SchemaGuidedEditableObjectRow = {
  key: string;
  value: JsonValue;
  rawValue: string;
  schema: JsonObject | undefined;
  fieldType: SchemaFieldType;
  required: boolean;
  unknown: boolean;
  scalarOptions: SchemaScalarOption[];
};

export type SchemaGuidedEditableArrayRow = {
  index: number;
  key: string;
  value: JsonValue;
  rawValue: string;
  schema: JsonObject | undefined;
  fieldType: SchemaFieldType;
  scalarOptions: SchemaScalarOption[];
};

export type SchemaGuidedObjectKeyOption = {
  key: string;
  label: string;
  required: boolean;
};

export type SchemaGuidedVariantOption = {
  index: number;
  label: string;
  active: boolean;
};

export function createSchemaGuidedValueEditorModel({
  rawText,
  schema,
  schemaOptions,
  maxDepth = 1,
}: {
  rawText: string;
  schema?: JsonObject;
  schemaOptions?: SchemaResolutionOptions;
  maxDepth?: number;
}): SchemaGuidedValueEditorModel {
  const options = editorSchemaOptions(schema, schemaOptions);
  const parseState = parseJsonText(rawText);
  const schemaTitle = typeof schema?.title === "string" ? schema.title : null;
  const schemaDescription =
    typeof schema?.description === "string" ? schema.description : null;

  if (!parseState.ok) {
    return {
      rawText,
      schemaTitle,
      schemaDescription,
      parseState,
      matchedVariantLabel: null,
      unknownKeyWarnings: [],
      rows: [],
      editableObjectRows: [],
      editableArrayRows: [],
      availableObjectKeys: [],
      variantOptions: [],
      activeVariantIndex: null,
    };
  }

  const valueSchema = schemaForValue(schema, parseState.value, options);
  const activeVariant = activeSchemaVariant(schema, parseState.value, options);
  const variantOptions = schemaGuidedVariantOptions({
    schema,
    value: parseState.value,
    schemaOptions: options,
  });
  const rows = createSchemaValueRows({
    value: parseState.value,
    schema: valueSchema,
    schemaOptions: options,
    maxDepth,
  });

  return {
    rawText,
    schemaTitle,
    schemaDescription,
    parseState,
    matchedVariantLabel: activeVariant?.label ?? null,
    unknownKeyWarnings: rows
      .filter((row) => row.unknown)
      .map((row) => ({ key: row.key, path: rowPath(row) })),
    rows,
    editableObjectRows: editableObjectRowsForValue({
      value: parseState.value,
      schema: valueSchema,
      schemaOptions: options,
    }),
    editableArrayRows: editableArrayRowsForValue({
      value: parseState.value,
      schema: valueSchema,
      schemaOptions: options,
    }),
    availableObjectKeys: availableObjectKeysForValue({
      value: parseState.value,
      schema: valueSchema,
      schemaOptions: options,
    }),
    variantOptions,
    activeVariantIndex: activeVariant?.index ?? null,
  };
}

export function editorSchemaOptions(
  schema: JsonObject | undefined,
  schemaOptions: SchemaResolutionOptions | undefined,
): SchemaResolutionOptions {
  return {
    ...schemaOptions,
    rootSchema: schemaOptions?.rootSchema ?? schema,
    baseUri:
      schemaOptions?.baseUri ??
      (typeof schema?.$id === "string" ? schema.$id : undefined),
  };
}

export function schemaGuidedVariants(
  schema: JsonObject | undefined,
  schemaOptions: SchemaResolutionOptions,
): Array<{ index: number; schema: JsonObject; label: string }> {
  const resolvedSchema = resolveSchema(schema, schemaOptions);
  const rawVariants = Array.isArray(resolvedSchema?.oneOf)
    ? resolvedSchema.oneOf
    : Array.isArray(resolvedSchema?.anyOf)
      ? resolvedSchema.anyOf
      : [];

  return rawVariants.flatMap((variant, index) => {
    const resolvedVariant = resolveSchema(variant, schemaOptions);
    return resolvedVariant
      ? [
          {
            index,
            schema: resolvedVariant,
            label: schemaVariantLabel(resolvedVariant, index),
          },
        ]
      : [];
  });
}

function editableObjectRowsForValue({
  value,
  schema,
  schemaOptions,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions;
}): SchemaGuidedEditableObjectRow[] {
  if (!isJsonObject(value)) return [];

  const metadata = schemaMetadataForObject(schema, schemaOptions);
  const closed = schemaConstraints(schema, schemaOptions).includes("closed");

  return Object.entries(value).map(([key, item]) => {
    const propertySchema = schemaForProperty(key, schema, schemaOptions);
    const resolvedPropertySchema = schemaForValue(
      propertySchema,
      item,
      schemaOptions,
    );
    return {
      key,
      value: item,
      rawValue: jsonValueToInputValue(item),
      schema: resolvedPropertySchema,
      fieldType: fieldTypeForValue(item, resolvedPropertySchema, schemaOptions),
      required: metadata.requiredKeys.has(key),
      unknown: propertySchema === undefined && closed,
      scalarOptions: schemaScalarOptions(resolvedPropertySchema, schemaOptions),
    };
  });
}

function editableArrayRowsForValue({
  value,
  schema,
  schemaOptions,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions;
}): SchemaGuidedEditableArrayRow[] {
  if (!Array.isArray(value)) return [];

  const itemSchema = schemaForArrayItem(schema, schemaOptions);
  return value.map((item, index) => {
    const resolvedItemSchema = schemaForValue(itemSchema, item, schemaOptions);
    return {
      index,
      key: `[${index}]`,
      value: item,
      rawValue: jsonValueToInputValue(item),
      schema: resolvedItemSchema,
      fieldType: fieldTypeForValue(item, resolvedItemSchema, schemaOptions),
      scalarOptions: schemaScalarOptions(resolvedItemSchema, schemaOptions),
    };
  });
}

function availableObjectKeysForValue({
  value,
  schema,
  schemaOptions,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions;
}): SchemaGuidedObjectKeyOption[] {
  if (!isJsonObject(value)) return [];

  const metadata = schemaMetadataForObject(schema, schemaOptions);
  return [...metadata.properties.entries()]
    .filter(([key]) => !Object.prototype.hasOwnProperty.call(value, key))
    .map(([key, propertySchema]) => ({
      key,
      label: schemaLabel(key, propertySchema),
      required: metadata.requiredKeys.has(key),
    }));
}

function schemaGuidedVariantOptions({
  schema,
  value,
  schemaOptions,
}: {
  schema: JsonObject | undefined;
  value: JsonValue;
  schemaOptions: SchemaResolutionOptions;
}): SchemaGuidedVariantOption[] {
  const activeVariant = activeSchemaVariant(schema, value, schemaOptions);
  return schemaGuidedVariants(schema, schemaOptions).map((variant) => ({
    index: variant.index,
    label: variant.label,
    active: variant.index === activeVariant?.index,
  }));
}

function schemaVariantLabel(schema: JsonObject, index: number) {
  if (typeof schema.title === "string") return schema.title;

  const properties = schema.properties;
  if (isJsonObject(properties)) {
    for (const [key, propertySchema] of Object.entries(properties)) {
      const constValue = schemaConstValue(
        isJsonObject(propertySchema) ? propertySchema : undefined,
      );
      if (constValue === undefined) continue;
      if (key === "mode" && constValue === "http") return "HTTP fetch";
      if (key === "mode" && constValue === "browser") return "Browser fetch";
      return String(constValue);
    }
  }

  return `Variante ${index + 1}`;
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

function fieldTypeForValue(
  value: JsonValue,
  schema: JsonObject | undefined,
  schemaOptions: SchemaResolutionOptions,
): SchemaFieldType {
  if (schema) return schemaFieldTypeFromSchema(schema, schemaOptions);
  if (typeof value === "number") return "number";
  if (typeof value === "boolean") return "boolean";
  if (Array.isArray(value) || isJsonObject(value)) return "json";
  return "string";
}

function jsonValueToInputValue(value: JsonValue | undefined): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return JSON.stringify(value);
}

function schemaLabel(key: string, schema: JsonObject | undefined) {
  return typeof schema?.title === "string" ? schema.title : key;
}

function parseJsonText(
  rawText: string,
): SchemaGuidedValueEditorModel["parseState"] {
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

function rowPath(row: SchemaValueTreeRowModel) {
  return [...row.ancestorIds.map(rowKeyFromId), row.key].join(".");
}

function rowKeyFromId(id: string) {
  const separatorIndex = id.lastIndexOf(":");
  const key = separatorIndex >= 0 ? id.slice(separatorIndex + 1) : id;
  return key || id;
}
