import { Badge } from "@/components/reui/badge";
import { Textarea } from "@/components/ui/textarea";
import {
  activeSchemaVariant,
  schemaForValue,
  type JsonObject,
  type SchemaResolutionOptions,
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
};

export type SchemaGuidedUnknownKeyWarning = {
  key: string;
  path: string;
};

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
          <SchemaValueTable
            value={model.parseState.value}
            schema={schema}
            schemaOptions={schemaOptions}
            maxDepth={previewMaxDepth}
          />
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
    };
  }

  const valueSchema = schemaForValue(schema, parseState.value, options);
  const activeVariant = activeSchemaVariant(schema, parseState.value, options);
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
  };
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
