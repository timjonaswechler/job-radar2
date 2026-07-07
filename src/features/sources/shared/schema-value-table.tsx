import { useState } from "react";

import { ChevronDownIcon, ChevronUpIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  isJsonObject,
  schemaFieldType,
  type JsonObject,
} from "@/features/sources/shared/source-config-schema";
import type { JsonValue } from "@/lib/api/sources";

type SchemaValueTableProps = {
  value: JsonValue | undefined;
  schema?: JsonValue;
  maxDepth?: number;
  className?: string;
};

type SchemaValuePreviewProps = SchemaValueTableProps & {
  title: string;
  description?: string;
};

type OptionalSchemaValuePreviewProps = Omit<
  SchemaValuePreviewProps,
  "value"
> & {
  value: JsonValue | null | undefined;
};

type RowModel = {
  key: string;
  value: JsonValue;
  schema: JsonObject | undefined;
  required: boolean;
};

type TreeRowModel = RowModel & {
  id: string;
  depth: number;
  ancestorIds: string[];
  expandable: boolean;
};

export function SchemaValuePreview({
  title,
  description,
  value,
  schema,
  maxDepth,
  className,
}: SchemaValuePreviewProps) {
  return (
    <section className="grid gap-3">
      <div className="min-w-0">
        <p className="font-medium">{title}</p>
        {description ? (
          <p className="text-xs text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <SchemaValueTable
        value={value}
        schema={schema}
        maxDepth={maxDepth}
        className={className}
      />
    </section>
  );
}

export function OptionalSchemaValuePreview({
  value,
  ...props
}: OptionalSchemaValuePreviewProps) {
  if (value === null || value === undefined) return null;
  return <SchemaValuePreview value={value} {...props} />;
}

export function SchemaValueTable({
  value,
  schema,
  maxDepth = 6,
  className,
}: SchemaValueTableProps) {
  if (value === undefined) {
    return <p className="text-xs text-muted-foreground">Nicht gesetzt.</p>;
  }

  return (
    <SchemaValueRowsTable
      value={value}
      schema={asSchemaObject(schema)}
      maxDepth={maxDepth}
      className={className}
    />
  );
}

function SchemaValueRowsTable({
  value,
  schema,
  maxDepth,
  className,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
  maxDepth: number;
  className?: string;
}) {
  const rows = treeRowsForValue(value, schema, maxDepth);
  const [expandedRows, setExpandedRows] = useState<Set<string>>(
    () =>
      new Set(
        rows
          .filter((row) => row.expandable && row.depth < 2)
          .map((row) => row.id),
      ),
  );

  if (!rows.length) {
    return <ScalarValue value={value} schema={schema} />;
  }

  const visibleRows = rows.filter((row) =>
    row.ancestorIds.every((ancestorId) => expandedRows.has(ancestorId)),
  );

  const toggleRow = (rowId: string) => {
    setExpandedRows((current) => {
      const next = new Set(current);
      if (next.has(rowId)) {
        next.delete(rowId);
      } else {
        next.add(rowId);
      }
      return next;
    });
  };

  return (
    <Table className={tableClassName(className)}>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead className="h-8 w-[34%] bg-muted/40 px-2">Key</TableHead>
          <TableHead className="h-8 bg-muted/40 px-2">Value</TableHead>
          <TableHead className="h-8 w-24 bg-muted/40 px-2">Type</TableHead>
          <TableHead className="h-8 w-[22%] bg-muted/40 px-2">Rule</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody className="[&_tr:last-child]:border-b-0">
        {visibleRows.map((row) => (
          <SchemaValueRow
            key={row.id}
            row={row}
            expanded={expandedRows.has(row.id)}
            onToggle={() => toggleRow(row.id)}
          />
        ))}
      </TableBody>
    </Table>
  );
}

function SchemaValueRow({
  row,
  expanded,
  onToggle,
}: {
  row: TreeRowModel;
  expanded: boolean;
  onToggle: () => void;
}) {
  const nested = isNestedValue(row.value);
  const typeLabel = valueTypeLabel(row.value, row.schema);
  const schemaTitle =
    typeof row.schema?.title === "string" ? row.schema.title : null;
  const summary = nested ? nestedSummary(row.value) : null;

  return (
    <TableRow className="hover:bg-transparent">
      <TableCell className="whitespace-normal px-2 py-1.5 align-top font-mono">
        <div className="flex min-w-0 items-start gap-1">
          <span
            aria-hidden="true"
            className="shrink-0"
            style={{ width: row.depth * 18 }}
          />
          {row.expandable ? (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="shrink-0 text-muted-foreground hover:bg-transparent"
              onClick={onToggle}
              aria-expanded={expanded}
              aria-label={
                expanded ? "Details einklappen" : "Details ausklappen"
              }
            >
              {expanded ? (
                <ChevronUpIcon aria-hidden="true" />
              ) : (
                <ChevronDownIcon aria-hidden="true" />
              )}
            </Button>
          ) : (
            <span aria-hidden="true" className="size-6 shrink-0" />
          )}
          <span className="flex min-w-0 flex-col gap-1">
            <span className="truncate">{row.key}</span>
            {schemaTitle ? (
              <span className="font-sans text-muted-foreground">
                {schemaTitle}
              </span>
            ) : null}
          </span>
        </div>
      </TableCell>
      <TableCell className="whitespace-normal px-2 py-1.5 align-top">
        {nested ? (
          <span className="text-muted-foreground">{summary}</span>
        ) : (
          <ScalarValue value={row.value} schema={row.schema} />
        )}
      </TableCell>
      <TableCell className="whitespace-normal px-2 py-1.5 align-top text-muted-foreground">
        {typeLabel}
      </TableCell>
      <TableCell className="whitespace-normal px-2 py-1.5 align-top">
        <SchemaRule schema={row.schema} required={row.required} />
      </TableCell>
    </TableRow>
  );
}

function SchemaRule({
  schema,
  required,
}: {
  schema: JsonObject | undefined;
  required: boolean;
}) {
  const options = schemaOptions(schema);
  const constraints = schemaConstraints(schema);

  if (!required && !options.length && !constraints.length) {
    return <span className="text-muted-foreground">—</span>;
  }

  return (
    <div className="flex flex-wrap gap-1">
      {required ? <Badge variant="warning-light">Pflicht</Badge> : null}
      {options.map((option) => (
        <Badge key={option} variant="outline">
          {option}
        </Badge>
      ))}
      {constraints.map((constraint) => (
        <Badge key={constraint} variant="secondary">
          {constraint}
        </Badge>
      ))}
    </div>
  );
}

function ScalarValue({
  value,
  schema,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
}) {
  if (value === null)
    return <span className="text-muted-foreground">null</span>;
  if (typeof value === "boolean") {
    return (
      <Badge variant={value ? "success-outline" : "outline"}>
        {String(value)}
      </Badge>
    );
  }
  if (typeof value === "number")
    return <span className="font-mono">{value}</span>;

  const displayValue =
    typeof value === "string" ? value : JSON.stringify(value);
  const options = schemaOptions(schema);
  const knownOption = options.includes(displayValue);

  return (
    <span className="inline-flex min-w-0 flex-wrap items-center gap-1">
      <code className="break-all rounded-sm bg-muted px-1 py-0.5">
        {displayValue}
      </code>
      {knownOption ? <Badge variant="outline">vordefiniert</Badge> : null}
    </span>
  );
}

function treeRowsForValue(
  value: JsonValue,
  schema: JsonObject | undefined,
  maxDepth: number,
  depth = 0,
  parentId = "root",
  ancestorIds: string[] = [],
): TreeRowModel[] {
  return rowsForValue(value, schema).flatMap((row, index) => {
    const id = `${parentId}/${index}:${row.key}`;
    const expandable =
      isNestedValue(row.value) &&
      depth < maxDepth &&
      rowsForValue(row.value, row.schema).length > 0;
    const treeRow: TreeRowModel = {
      ...row,
      id,
      depth,
      ancestorIds,
      expandable,
    };
    const childRows = expandable
      ? treeRowsForValue(row.value, row.schema, maxDepth, depth + 1, id, [
          ...ancestorIds,
          id,
        ])
      : [];

    return [treeRow, ...childRows];
  });
}

function rowsForValue(
  value: JsonValue,
  schema: JsonObject | undefined,
): RowModel[] {
  if (Array.isArray(value)) {
    const itemSchema = asSchemaObject(schema?.items);
    return value.map((item, index) => ({
      key: `[${index}]`,
      value: item,
      schema: itemSchema,
      required: false,
    }));
  }

  if (isJsonObject(value)) {
    const objectSchemas = flattenSchemaObjects(schema);
    const requiredKeys = new Set(
      objectSchemas.flatMap((objectSchema) =>
        Array.isArray(objectSchema.required)
          ? objectSchema.required.filter(
              (key): key is string => typeof key === "string",
            )
          : [],
      ),
    );

    return Object.entries(value).map(([key, item]) => ({
      key,
      value: item,
      schema: schemaForProperty(key, objectSchemas),
      required: requiredKeys.has(key),
    }));
  }

  return [];
}

function schemaForProperty(
  key: string,
  schemas: JsonObject[],
): JsonObject | undefined {
  for (const schema of schemas) {
    const properties = schema.properties;
    if (!isJsonObject(properties)) continue;
    const property = properties[key];
    if (isJsonObject(property)) return property;
  }
  return undefined;
}

function flattenSchemaObjects(schema: JsonObject | undefined): JsonObject[] {
  if (!schema) return [];
  const schemas = [schema];
  const allOf = schema.allOf;
  if (Array.isArray(allOf)) {
    for (const child of allOf) {
      if (isJsonObject(child)) schemas.push(...flattenSchemaObjects(child));
    }
  }
  return schemas;
}

function schemaOptions(schema: JsonObject | undefined) {
  if (!schema) return [];

  const values: string[] = [];
  if ("const" in schema && isScalarSchemaValue(schema.const)) {
    values.push(String(schema.const));
  }

  if (Array.isArray(schema.enum)) {
    for (const value of schema.enum) {
      if (isScalarSchemaValue(value)) values.push(String(value));
    }
  }

  return [...new Set(values)];
}

function schemaConstraints(schema: JsonObject | undefined) {
  if (!schema) return [];
  const constraints: string[] = [];

  if (typeof schema.format === "string") constraints.push(schema.format);
  if (typeof schema.pattern === "string") constraints.push("pattern");
  if (typeof schema.minimum === "number")
    constraints.push(`min ${schema.minimum}`);
  if (typeof schema.maximum === "number")
    constraints.push(`max ${schema.maximum}`);
  if (schema.additionalProperties === false) constraints.push("closed");

  return constraints;
}

function isNestedValue(value: JsonValue) {
  return Array.isArray(value) || isJsonObject(value);
}

function nestedSummary(value: JsonValue) {
  if (Array.isArray(value)) return `Array · ${value.length} Einträge`;
  if (isJsonObject(value)) return `Object · ${Object.keys(value).length} Keys`;
  return "";
}

function valueTypeLabel(value: JsonValue, schema: JsonObject | undefined) {
  const schemaType = schema ? schemaFieldType(schema) : null;
  if (schemaType === "json") return Array.isArray(value) ? "array" : "object";
  if (schemaType) return schemaType;
  if (Array.isArray(value)) return "array";
  if (value === null) return "null";
  return typeof value;
}

function asSchemaObject(schema: JsonValue | undefined): JsonObject | undefined {
  return isJsonObject(schema) ? schema : undefined;
}

function isScalarSchemaValue(
  value: unknown,
): value is string | number | boolean {
  return (
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  );
}

function tableClassName(className: string | undefined) {
  return [
    "border-separate border-spacing-0 rounded-md border border-border text-xs",
    "[&_td]:border-r [&_td]:border-border [&_td:last-child]:border-r-0",
    "[&_th]:border-r [&_th]:border-border [&_th:last-child]:border-r-0",
    className,
  ]
    .filter(Boolean)
    .join(" ");
}
