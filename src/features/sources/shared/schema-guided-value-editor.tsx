import { useState } from "react";

import { PlusIcon, Trash2Icon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import {
  activeSchemaVariant,
  isJsonObject,
  resolveSchema,
  schemaConstraints,
  schemaDefaultValue,
  schemaFieldTypeFromSchema,
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
import { SchemaValueTable } from "@/features/sources/shared/schema-value-table";
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

export type SchemaGuidedObjectEdit =
  | { type: "add-property"; key: string }
  | { type: "remove-property"; key: string }
  | { type: "set-property-value"; key: string; rawValue: string }
  | { type: "select-variant"; variantIndex: number };

export type SchemaGuidedObjectEditResult =
  | { ok: true; rawText: string }
  | { ok: false; rawText: string; error: string };

type SchemaGuidedValueEditorProps = {
  value: string;
  onChange: (value: string) => void;
  ariaLabel: string;
  schema?: JsonObject;
  schemaOptions?: SchemaResolutionOptions;
  disabled?: boolean;
  previewMaxDepth?: number;
  textareaClassName?: string;
};

export function SchemaGuidedValueEditor({
  value,
  onChange,
  ariaLabel,
  schema,
  schemaOptions,
  disabled,
  previewMaxDepth = 1,
  textareaClassName,
}: SchemaGuidedValueEditorProps) {
  const model = createSchemaGuidedValueEditorModel({
    rawText: value,
    schema,
    schemaOptions,
    maxDepth: previewMaxDepth,
  });

  return (
    <div className="flex flex-col gap-2">
      <Textarea
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="JSON-Wert"
        aria-label={ariaLabel}
        aria-invalid={!model.parseState.ok}
        disabled={disabled}
        className={textareaClassName}
      />
      <div className="flex flex-col gap-2 px-2 pb-2">
        <SchemaGuidanceSummary model={model} />
        {model.parseState.ok ? (
          <>
            <SchemaGuidedObjectRowsEditor
              model={model}
              schema={schema}
              schemaOptions={schemaOptions}
              disabled={disabled}
              onChange={onChange}
            />
            <SchemaValueTable
              value={model.parseState.value}
              schema={schema}
              schemaOptions={schemaOptions}
              maxDepth={previewMaxDepth}
            />
          </>
        ) : (
          <p className="text-xs text-muted-foreground">
            Vorschau und Schema-Hinweise erscheinen, sobald der JSON-Wert gültig
            ist. Der aktuelle Text bleibt erhalten.
          </p>
        )}
      </div>
    </div>
  );
}

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
    availableObjectKeys: availableObjectKeysForValue({
      value: parseState.value,
      schema: valueSchema,
      schemaOptions: options,
    }),
    variantOptions,
    activeVariantIndex: activeVariant?.index ?? null,
  };
}

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

function SchemaGuidedObjectRowsEditor({
  model,
  schema,
  schemaOptions,
  disabled,
  onChange,
}: {
  model: SchemaGuidedValueEditorModel;
  schema: JsonObject | undefined;
  schemaOptions: SchemaResolutionOptions | undefined;
  disabled: boolean | undefined;
  onChange: (value: string) => void;
}) {
  const [newKey, setNewKey] = useState("");

  if (!model.parseState.ok || !isJsonObject(model.parseState.value)) return null;

  const selectedKnownKey = model.availableObjectKeys.some(
    (option) => option.key === newKey,
  )
    ? newKey
    : null;
  const applyEdit = (edit: SchemaGuidedObjectEdit) => {
    const result = applySchemaGuidedObjectEdit({
      rawText: model.rawText,
      schema,
      schemaOptions,
      edit,
    });
    if (result.ok) onChange(result.rawText);
  };
  const addKey = () => {
    const key = newKey.trim();
    if (!key) return;
    applyEdit({ type: "add-property", key });
    setNewKey("");
  };

  return (
    <div className="flex flex-col gap-2">
      {model.variantOptions.length > 1 ? (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs font-medium">Variante</span>
          <Select
            items={model.variantOptions.map((option) => ({
              value: String(option.index),
              label: option.label,
            }))}
            modal={false}
            value={
              model.activeVariantIndex === null
                ? null
                : String(model.activeVariantIndex)
            }
            onValueChange={(value) => {
              if (value !== null) {
                applyEdit({ type: "select-variant", variantIndex: Number(value) });
              }
            }}
          >
            <SelectTrigger
              className="h-8 min-w-44 text-xs"
              aria-label="Schema-Variante auswählen"
              disabled={disabled}
            >
              <SelectValue placeholder="Variante wählen" />
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false}>
              <SelectGroup>
                {model.variantOptions.map((option) => (
                  <SelectItem key={option.index} value={String(option.index)}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
      ) : null}

      {model.editableObjectRows.length ? (
        <Table className={compactTableClassName()}>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead className="h-8 w-[32%] bg-muted/40 px-2">Key</TableHead>
              <TableHead className="h-8 bg-muted/40 px-2">Wert</TableHead>
              <TableHead className="h-8 w-[22%] bg-muted/40 px-2">Regel</TableHead>
              <TableHead className="h-8 w-10 bg-muted/40 px-1 text-right">
                <span className="sr-only">Aktionen</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody className="[&_tr:last-child]:border-b-0">
            {model.editableObjectRows.map((row) => (
              <TableRow key={row.key} className="hover:bg-transparent">
                <TableCell className="whitespace-normal px-2 py-1.5 align-top font-mono">
                  {row.key}
                </TableCell>
                <TableCell className="p-0 align-top">
                  <SchemaGuidedObjectValueCell
                    row={row}
                    disabled={disabled}
                    onChange={(rawValue) =>
                      applyEdit({
                        type: "set-property-value",
                        key: row.key,
                        rawValue,
                      })
                    }
                  />
                </TableCell>
                <TableCell className="whitespace-normal px-2 py-1.5 align-top">
                  <SchemaGuidedObjectRowRule row={row} />
                </TableCell>
                <TableCell className="p-1 align-top text-right">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={() =>
                      applyEdit({ type: "remove-property", key: row.key })
                    }
                    disabled={disabled || row.required}
                    title={
                      row.required
                        ? "Pflicht-Key kann nicht entfernt werden"
                        : "Key entfernen"
                    }
                  >
                    <Trash2Icon aria-hidden="true" />
                    <span className="sr-only">Key entfernen</span>
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      ) : null}

      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={newKey}
          onChange={(event) => setNewKey(event.target.value)}
          placeholder="Object-Key"
          aria-label="Object-Key hinzufügen"
          disabled={disabled}
          className="h-8 min-w-36 flex-1 text-xs"
        />
        {model.availableObjectKeys.length ? (
          <Select
            items={model.availableObjectKeys.map((option) => ({
              value: option.key,
              label: option.key,
            }))}
            modal={false}
            value={selectedKnownKey}
            onValueChange={(value) => {
              if (value) setNewKey(value);
            }}
          >
            <SelectTrigger
              className="h-8 min-w-40 text-xs"
              aria-label="Schema-Key auswählen"
              disabled={disabled}
            >
              <SelectValue placeholder="Schema-Key" />
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false}>
              <SelectGroup>
                {model.availableObjectKeys.map((option) => (
                  <SelectItem key={option.key} value={option.key}>
                    {option.key}
                    {option.required ? " · Pflicht" : ""}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        ) : null}
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={addKey}
          disabled={disabled || !newKey.trim()}
        >
          <PlusIcon data-icon="inline-start" aria-hidden="true" />
          Key hinzufügen
        </Button>
      </div>
    </div>
  );
}

function SchemaGuidedObjectValueCell({
  row,
  disabled,
  onChange,
}: {
  row: SchemaGuidedEditableObjectRow;
  disabled: boolean | undefined;
  onChange: (rawValue: string) => void;
}) {
  if (row.scalarOptions.length) {
    return (
      <Select
        items={row.scalarOptions.map((option) => ({
          value: jsonValueToInputValue(option.value),
          label: option.label,
        }))}
        modal={false}
        value={jsonValueToInputValue(row.value) || null}
        onValueChange={(value) => {
          if (value !== null) onChange(value);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 text-xs shadow-none ring-0 focus:ring-0"
          aria-label={`Wert für ${row.key}`}
          disabled={disabled}
        >
          <SelectValue placeholder="Wert wählen" />
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false}>
          <SelectGroup>
            {row.scalarOptions.map((option) => (
              <SelectItem
                key={jsonValueToInputValue(option.value)}
                value={jsonValueToInputValue(option.value)}
              >
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    );
  }

  if (row.fieldType === "boolean") {
    return (
      <Select
        items={booleanOptions}
        modal={false}
        value={typeof row.value === "boolean" ? String(row.value) : null}
        onValueChange={(value) => {
          if (value) onChange(value);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 text-xs shadow-none ring-0 focus:ring-0"
          aria-label={`Wert für ${row.key}`}
          disabled={disabled}
        >
          <SelectValue placeholder="Boolean wählen" />
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false}>
          <SelectGroup>
            {booleanOptions.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    );
  }

  if (row.fieldType === "json") {
    return (
      <Textarea
        value={jsonValueToInputValue(row.value)}
        onChange={(event) => onChange(event.target.value)}
        aria-label={`Wert für ${row.key}`}
        disabled={disabled}
        className="min-h-12 rounded-none border-0 bg-transparent px-2 py-1.5 font-mono text-xs shadow-none ring-0 focus-visible:ring-0"
      />
    );
  }

  return (
    <Input
      value={jsonValueToInputValue(row.value)}
      onChange={(event) => onChange(event.target.value)}
      aria-label={`Wert für ${row.key}`}
      disabled={disabled}
      type={row.fieldType === "number" ? "number" : inputTypeForSchema(row.key, row.schema)}
      className="h-8 rounded-none border-0 bg-transparent text-xs shadow-none ring-0 focus-visible:ring-0"
    />
  );
}

function SchemaGuidedObjectRowRule({
  row,
}: {
  row: SchemaGuidedEditableObjectRow;
}) {
  if (!row.required && !row.unknown && !row.scalarOptions.length) {
    return <span className="text-muted-foreground">—</span>;
  }

  return (
    <div className="flex flex-wrap gap-1">
      {row.required ? <Badge variant="warning-light">Pflicht</Badge> : null}
      {row.unknown ? <Badge variant="warning-light">not in schema</Badge> : null}
      {row.scalarOptions.map((option) => (
        <Badge key={jsonValueToInputValue(option.value)} variant="outline">
          {option.label}
        </Badge>
      ))}
    </div>
  );
}

function SchemaGuidanceSummary({
  model,
}: {
  model: SchemaGuidedValueEditorModel;
}) {
  const hasSummary =
    model.schemaTitle ||
    model.schemaDescription ||
    model.matchedVariantLabel ||
    model.unknownKeyWarnings.length ||
    !model.parseState.ok;

  if (!hasSummary) return null;

  return (
    <div className="flex flex-col gap-1 text-xs">
      {model.schemaTitle || model.schemaDescription ? (
        <div className="flex flex-col gap-0.5">
          {model.schemaTitle ? <p className="font-medium">{model.schemaTitle}</p> : null}
          {model.schemaDescription ? (
            <p className="text-muted-foreground">{model.schemaDescription}</p>
          ) : null}
        </div>
      ) : null}
      <div className="flex flex-wrap gap-1">
        {model.matchedVariantLabel ? (
          <Badge variant="secondary">{model.matchedVariantLabel}</Badge>
        ) : null}
        {!model.parseState.ok ? (
          <Badge variant="warning-light">JSON noch ungültig</Badge>
        ) : null}
        {model.unknownKeyWarnings.map((warning) => (
          <Badge key={warning.path} variant="warning-light">
            not in schema: {warning.path}
          </Badge>
        ))}
      </div>
    </div>
  );
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

function schemaGuidedVariants(
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
      ? [{ index, schema: resolvedVariant, label: schemaVariantLabel(resolvedVariant, index) }]
      : [];
  });
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
      (metadata.requiredKeys.has(key) || schemaDefaultValue(propertySchema, schemaOptions) !== undefined)
    ) {
      value[key] = defaultValueForSchema(propertySchema, schemaOptions);
    }
  }
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
  if (fieldType === "json") return schema?.type === "array" ? [] : {};
  return "";
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
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

function stringifyJson(value: JsonValue) {
  return JSON.stringify(value, null, 2);
}

function schemaLabel(key: string, schema: JsonObject | undefined) {
  return typeof schema?.title === "string" ? schema.title : key;
}

function inputTypeForSchema(key: string, schema: JsonObject | undefined) {
  if (schema?.format === "uri" || /url$/i.test(key)) return "url";
  return "text";
}

function compactTableClassName() {
  return [
    "border-separate border-spacing-0 rounded-md border border-border text-xs",
    "[&_td]:border-r [&_td]:border-border [&_td:last-child]:border-r-0",
    "[&_th]:border-r [&_th]:border-border [&_th:last-child]:border-r-0",
  ].join(" ");
}

const booleanOptions = [
  { value: "true", label: "Ja / true" },
  { value: "false", label: "Nein / false" },
];

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

function editorSchemaOptions(
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

function rowPath(row: SchemaValueTreeRowModel) {
  return [...row.ancestorIds.map(rowKeyFromId), row.key].join(".");
}

function rowKeyFromId(id: string) {
  const separatorIndex = id.lastIndexOf(":");
  const key = separatorIndex >= 0 ? id.slice(separatorIndex + 1) : id;
  return key || id;
}
