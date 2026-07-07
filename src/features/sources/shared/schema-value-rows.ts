import {
  activeSchemaVariant,
  isJsonObject,
  schemaConstraints,
  schemaForArrayItem,
  schemaForProperty,
  schemaForValue,
  schemaMetadataForObject,
  type JsonObject,
  type SchemaResolutionOptions,
} from "@/features/sources/shared/schema-introspection";
import type { JsonValue } from "@/lib/api/sources";

export type SchemaValueRowModel = {
  key: string;
  value: JsonValue;
  schema: JsonObject | undefined;
  required: boolean;
  unknown: boolean;
  variantLabel: string | null;
};

export type SchemaValueTreeRowModel = SchemaValueRowModel & {
  id: string;
  depth: number;
  ancestorIds: string[];
  expandable: boolean;
};

export function createSchemaValueRows({
  value,
  schema,
  schemaOptions,
  maxDepth,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions;
  maxDepth: number;
}): SchemaValueTreeRowModel[] {
  return treeRowsForValue(
    value,
    schemaForValue(schema, value, schemaOptions),
    schemaOptions,
    maxDepth,
  );
}

function treeRowsForValue(
  value: JsonValue,
  schema: JsonObject | undefined,
  schemaOptions: SchemaResolutionOptions,
  maxDepth: number,
  depth = 0,
  parentId = "root",
  ancestorIds: string[] = [],
): SchemaValueTreeRowModel[] {
  return rowsForValue(
    value,
    schemaForValue(schema, value, schemaOptions),
    schemaOptions,
  ).flatMap((row, index) => {
    const id = `${parentId}/${index}:${row.key}`;
    const rowSchema = schemaForValue(row.schema, row.value, schemaOptions);
    const expandable =
      isNestedValue(row.value) &&
      depth < maxDepth &&
      rowsForValue(row.value, rowSchema, schemaOptions).length > 0;
    const treeRow: SchemaValueTreeRowModel = {
      ...row,
      schema: rowSchema,
      id,
      depth,
      ancestorIds,
      expandable,
    };
    const childRows = expandable
      ? treeRowsForValue(
          row.value,
          rowSchema,
          schemaOptions,
          maxDepth,
          depth + 1,
          id,
          [...ancestorIds, id],
        )
      : [];

    return [treeRow, ...childRows];
  });
}

function rowsForValue(
  value: JsonValue,
  schema: JsonObject | undefined,
  schemaOptions: SchemaResolutionOptions,
): SchemaValueRowModel[] {
  if (Array.isArray(value)) {
    const itemSchema = schemaForArrayItem(schema, schemaOptions);
    return value.map((item, index) => {
      const variant = activeSchemaVariant(itemSchema, item, schemaOptions);
      return {
        key: `[${index}]`,
        value: item,
        schema: variant?.schema ?? schemaForValue(itemSchema, item, schemaOptions),
        required: false,
        unknown: false,
        variantLabel: variant?.label ?? null,
      };
    });
  }

  if (isJsonObject(value)) {
    const metadata = schemaMetadataForObject(schema, schemaOptions);
    const closed = schemaConstraints(schema, schemaOptions).includes("closed");

    return Object.entries(value).map(([key, item]) => {
      const propertySchema = schemaForProperty(key, schema, schemaOptions);
      const variant = activeSchemaVariant(propertySchema, item, schemaOptions);
      return {
        key,
        value: item,
        schema:
          variant?.schema ?? schemaForValue(propertySchema, item, schemaOptions),
        required: metadata.requiredKeys.has(key),
        unknown: propertySchema === undefined && closed,
        variantLabel: variant?.label ?? null,
      };
    });
  }

  return [];
}

function isNestedValue(value: JsonValue) {
  return Array.isArray(value) || isJsonObject(value);
}
