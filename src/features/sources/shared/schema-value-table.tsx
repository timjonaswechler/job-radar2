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
  schemaConstraints,
  schemaFieldTypeFromSchema,
  schemaForValue,
  schemaScalarOptions,
  schemaScalarRules,
  type JsonObject,
  type SchemaCatalog,
  type SchemaResolutionOptions,
} from "@/features/sources/shared/schema-introspection";
import { profileDslSchemaCatalog } from "@/features/sources/shared/profile-dsl-schema-catalog";
import {
  createSchemaValueRows,
  type SchemaValueTreeRowModel,
} from "@/features/sources/shared/schema-value-rows";
import type { JsonValue } from "@/lib/api/sources";

type SchemaValueTableProps = {
  value: JsonValue | undefined;
  schema?: JsonValue;
  schemaRef?: string;
  schemaCatalog?: SchemaCatalog;
  schemaOptions?: SchemaResolutionOptions;
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

export function SchemaValuePreview({
  title,
  description,
  value,
  schema,
  schemaRef,
  schemaCatalog,
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
        schemaRef={schemaRef}
        schemaCatalog={schemaCatalog}
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
  schemaRef,
  schemaCatalog = profileDslSchemaCatalog,
  schemaOptions,
  maxDepth = 6,
  className,
}: SchemaValueTableProps) {
  if (value === undefined) {
    return <p className="text-xs text-muted-foreground">Nicht gesetzt.</p>;
  }

  const schemaContext = schemaContextForPreview({
    schema,
    schemaRef,
    schemaCatalog,
    schemaOptions,
  });

  return (
    <SchemaValueRowsTable
      value={value}
      schema={schemaContext.schema}
      schemaOptions={schemaContext.options}
      maxDepth={maxDepth}
      className={className}
    />
  );
}

function SchemaValueRowsTable({
  value,
  schema,
  schemaOptions,
  maxDepth,
  className,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions;
  maxDepth: number;
  className?: string;
}) {
  const rows = createSchemaValueRows({
    value,
    schema: schemaForValue(schema, value, schemaOptions),
    schemaOptions,
    maxDepth,
  });
  const [expandedRows, setExpandedRows] = useState<Set<string>>(
    () =>
      new Set(
        rows
          .filter((row) => row.expandable && row.depth < 2)
          .map((row) => row.id),
      ),
  );

  if (!rows.length) {
    return (
      <ScalarValue
        value={value}
        schema={schema}
        schemaOptions={schemaOptions}
      />
    );
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
            schemaOptions={schemaOptions}
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
  schemaOptions,
  onToggle,
}: {
  row: SchemaValueTreeRowModel;
  expanded: boolean;
  schemaOptions: SchemaResolutionOptions;
  onToggle: () => void;
}) {
  const nested = isNestedValue(row.value);
  const typeLabel = valueTypeLabel(row.value, row.schema, schemaOptions);
  const schemaTitle =
    typeof row.schema?.title === "string" ? row.schema.title : null;
  const summary = nested ? nestedSummary(row.value) : null;

  return (
    <TableRow className="hover:bg-transparent">
      <TableCell className="whitespace-normal px-2 py-1.5 align-center font-mono">
        <div className="flex min-w-0 items-center gap-1 ">
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
              className="shrink-0 text-muted-foreground hover:bg-background bg-muted"
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
            <span aria-hidden="true" className="size-6 shrink-0 " />
          )}
          <span className="flex min-w-0 flex-col gap-1">
            <span className="truncate">{row.key}</span>
            {schemaTitle ? (
              <span className="font-sans text-muted-foreground">
                {schemaTitle}
              </span>
            ) : null}
            {row.variantLabel ? (
              <Badge variant="secondary" className="w-fit font-sans">
                {row.variantLabel}
              </Badge>
            ) : null}
          </span>
        </div>
      </TableCell>
      <TableCell className="whitespace-normal px-2 py-1.5 align-top content-center">
        {nested ? (
          <span className="text-muted-foreground">{summary}</span>
        ) : (
          <ScalarValue
            value={row.value}
            schema={row.schema}
            schemaOptions={schemaOptions}
          />
        )}
      </TableCell>
      <TableCell className="whitespace-normal px-2 py-1.5 align-top text-muted-foreground content-center">
        {typeLabel}
      </TableCell>
      <TableCell className="whitespace-normal px-2 py-1.5 align-top content-center">
        <SchemaRule
          schema={row.schema}
          required={row.required}
          unknown={row.unknown}
          schemaOptions={schemaOptions}
        />
      </TableCell>
    </TableRow>
  );
}

function SchemaRule({
  schema,
  required,
  unknown,
  schemaOptions,
}: {
  schema: JsonObject | undefined;
  required: boolean;
  unknown: boolean;
  schemaOptions: SchemaResolutionOptions;
}) {
  const scalarRules = schemaScalarRules(schema, schemaOptions);
  const constraints = schemaConstraints(schema, schemaOptions);

  if (!required && !unknown && !scalarRules.length && !constraints.length) {
    return <span className="text-muted-foreground">—</span>;
  }

  return (
    <div className="flex flex-wrap gap-1 items-center">
      {required ? <Badge variant="warning-light">Pflicht</Badge> : null}
      {unknown ? <Badge variant="warning-light">not in schema</Badge> : null}
      {scalarRules.map((rule) => (
        <Badge
          key={`${rule.kind}:${rule.label}`}
          variant={rule.kind === "const" ? "secondary" : "outline"}
        >
          {rule.kind}: {rule.label}
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
  schemaOptions,
}: {
  value: JsonValue;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions;
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
  const options = schemaScalarOptions(schema, schemaOptions).map(
    (option) => option.label,
  );
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

function isNestedValue(value: JsonValue) {
  return Array.isArray(value) || isJsonObject(value);
}

function nestedSummary(value: JsonValue) {
  if (Array.isArray(value)) return `Array · ${value.length} Einträge`;
  if (isJsonObject(value)) return `Object · ${Object.keys(value).length} Keys`;
  return "";
}

function valueTypeLabel(
  value: JsonValue,
  schema: JsonObject | undefined,
  schemaOptions: SchemaResolutionOptions,
) {
  const schemaType = schema
    ? schemaFieldTypeFromSchema(schema, schemaOptions)
    : null;
  if (schemaType === "json") return Array.isArray(value) ? "array" : "object";
  if (schemaType) return schemaType;
  if (Array.isArray(value)) return "array";
  if (value === null) return "null";
  return typeof value;
}

function schemaContextForPreview({
  schema,
  schemaRef,
  schemaCatalog,
  schemaOptions,
}: {
  schema: JsonValue | undefined;
  schemaRef: string | undefined;
  schemaCatalog: SchemaCatalog;
  schemaOptions: SchemaResolutionOptions | undefined;
}): { schema: JsonObject | undefined; options: SchemaResolutionOptions } {
  if (schemaRef) {
    const resolvedSchema = schemaCatalog.resolveRef(schemaRef);
    if (resolvedSchema) {
      return {
        schema: resolvedSchema.schema,
        options: {
          ...schemaOptions,
          catalog: schemaOptions?.catalog ?? schemaCatalog,
          rootSchema: schemaOptions?.rootSchema ?? resolvedSchema.rootSchema,
          baseUri: schemaOptions?.baseUri ?? resolvedSchema.baseUri,
        },
      };
    }
  }

  const schemaObject = asSchemaObject(schema);
  return {
    schema: schemaObject,
    options: {
      ...schemaOptions,
      catalog: schemaOptions?.catalog ?? schemaCatalog,
      rootSchema: schemaOptions?.rootSchema ?? schemaObject,
      baseUri:
        schemaOptions?.baseUri ??
        (typeof schemaObject?.$id === "string" ? schemaObject.$id : undefined),
    },
  };
}

function asSchemaObject(schema: JsonValue | undefined): JsonObject | undefined {
  return isJsonObject(schema) ? schema : undefined;
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
