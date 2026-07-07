import { LockIcon, PlusIcon, Trash2Icon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxLabel,
  ComboboxList,
  ComboboxSeparator,
} from "@/components/ui/combobox";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
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
import { createEntryId } from "@/features/sources/add/source/source-add-model";
import {
  configEntryDescription,
  jsonValueToInputValue,
  schemaDefaultValue,
  schemaFieldType,
  type JsonObject,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { SchemaValueTable } from "@/features/sources/shared/schema-value-table";
import type { JsonValue } from "@/lib/api/sources";

type SourceConfigEditorProps = {
  entries: SourceConfigEntry[];
  schemaMetadata: SchemaMetadata;
  disabled: boolean;
  configErrors: string[];
  showErrors: boolean;
  onChange: (entries: SourceConfigEntry[]) => void;
};

type ConfigKeyOption = {
  key: string;
  label: string;
  required: boolean;
};

export function SourceConfigEditor({
  entries,
  schemaMetadata,
  disabled,
  configErrors,
  showErrors,
  onChange,
}: SourceConfigEditorProps) {
  const knownKeys = [...schemaMetadata.properties.keys()];
  const keyOptions = knownKeys.map((key): ConfigKeyOption => {
    const schema = schemaMetadata.properties.get(key);
    return {
      key,
      label: schemaTitle(key, schema),
      required: schemaMetadata.requiredKeys.has(key),
    };
  });

  const addEntry = () => {
    const unusedKnownKey = knownKeys.find(
      (key) => !entries.some((entry) => entry.key === key),
    );
    const propertySchema = unusedKnownKey
      ? schemaMetadata.properties.get(unusedKnownKey)
      : undefined;

    onChange([
      ...entries,
      {
        id: createEntryId(),
        key: unusedKnownKey ?? "",
        value: jsonValueToInputValue(schemaDefaultValue(propertySchema)),
      },
    ]);
  };

  const updateEntry = (id: string, patch: Partial<SourceConfigEntry>) => {
    onChange(
      entries.map((entry) => (entry.id === id ? { ...entry, ...patch } : entry)),
    );
  };

  const removeEntry = (id: string) => {
    onChange(entries.filter((entry) => entry.id !== id));
  };

  return (
    <FieldSet>
      <FieldLegend>Quellenkonfiguration</FieldLegend>
      <FieldDescription>
        Werte werden als schema-geführte Key/Value-Tabelle gepflegt und beim
        Speichern in <code>sourceConfig</code> geschrieben.
      </FieldDescription>
      {keyOptions.length ? <SchemaKeyLegend options={keyOptions} /> : null}
      <FieldGroup>
        {entries.length ? (
          <Field>
            <FieldLabel className="sr-only">Konfigurationswerte</FieldLabel>
            <SourceConfigTable
              entries={entries}
              keyOptions={keyOptions}
              schemaMetadata={schemaMetadata}
              disabled={disabled}
              onUpdate={updateEntry}
              onRemove={removeEntry}
            />
            <FieldDescription>
              Pflichtwerte stammen aus dem effektiven Profil-/Access-Path-Schema
              und können nicht direkt entfernt werden. Zusätzliche Keys bleiben
              als freie Konfiguration möglich.
            </FieldDescription>
          </Field>
        ) : (
          <SourceConfigEmptyState disabled={disabled} onAdd={addEntry} />
        )}

        {entries.length ? (
          <Button
            type="button"
            variant="outline"
            onClick={addEntry}
            disabled={disabled}
          >
            <PlusIcon data-icon="inline-start" aria-hidden="true" />
            Wert hinzufügen
          </Button>
        ) : null}

        {showErrors && configErrors.length ? (
          <FieldError>
            <ul className="list-inside list-disc">
              {configErrors.map((error) => (
                <li key={error}>{error}</li>
              ))}
            </ul>
          </FieldError>
        ) : null}
      </FieldGroup>
    </FieldSet>
  );
}

type SchemaKeyLegendProps = {
  options: ConfigKeyOption[];
};

function SchemaKeyLegend({ options }: SchemaKeyLegendProps) {
  return (
    <div className="flex flex-wrap gap-1">
      {options.map((option) => (
        <Badge
          key={option.key}
          variant={option.required ? "warning-light" : "outline"}
        >
          {option.key}
          {option.required ? " · Pflicht" : ""}
        </Badge>
      ))}
    </div>
  );
}

type SourceConfigTableProps = {
  entries: SourceConfigEntry[];
  keyOptions: ConfigKeyOption[];
  schemaMetadata: SchemaMetadata;
  disabled: boolean;
  onUpdate: (id: string, patch: Partial<SourceConfigEntry>) => void;
  onRemove: (id: string) => void;
};

function SourceConfigTable({
  entries,
  keyOptions,
  schemaMetadata,
  disabled,
  onUpdate,
  onRemove,
}: SourceConfigTableProps) {
  return (
    <Table className="border-separate border-spacing-0 rounded-md border border-border text-xs [&_td]:border-r [&_td]:border-border [&_td:last-child]:border-r-0 [&_th]:border-r [&_th]:border-border [&_th:last-child]:border-r-0">
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead className="h-8 w-[32%] bg-muted/40 px-2">Key</TableHead>
          <TableHead className="h-8 bg-muted/40 px-2">Wert</TableHead>
          <TableHead className="h-8 w-28 bg-muted/40 px-2">Typ</TableHead>
          <TableHead className="h-8 w-12 bg-muted/40 px-1 text-right">
            <span className="sr-only">Aktionen</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody className="[&_tr:last-child]:border-b-0">
        {entries.map((entry, index) => {
          const propertySchema = schemaMetadata.properties.get(entry.key);
          const required = schemaMetadata.requiredKeys.has(entry.key);
          const fieldType = schemaFieldType(propertySchema);
          const description = entry.key
            ? configEntryDescription(entry.key, propertySchema, required)
            : "Freier Konfigurationswert.";

          return (
            <TableRow
              key={entry.id}
              className="hover:bg-transparent"
              title={description}
            >
              <TableCell className="whitespace-normal p-0 align-top">
                <ConfigKeyCell
                  entry={entry}
                  index={index}
                  keyOptions={keyOptions}
                  required={required}
                  disabled={disabled}
                  onChange={(key) => onUpdate(entry.id, { key })}
                />
              </TableCell>
              <TableCell className="whitespace-normal p-0 align-top">
                <ConfigValueCell
                  entry={entry}
                  index={index}
                  propertySchema={propertySchema}
                  disabled={disabled}
                  onChange={(value) => onUpdate(entry.id, { value })}
                />
              </TableCell>
              <TableCell className="whitespace-normal px-2 py-1.5 align-top text-muted-foreground">
                <div className="flex flex-col gap-1">
                  <span>{schemaFieldTypeLabel(fieldType)}</span>
                  {required ? <Badge variant="warning-light">Pflicht</Badge> : null}
                </div>
              </TableCell>
              <TableCell className="p-1 align-top text-right">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => onRemove(entry.id)}
                  disabled={disabled || required}
                  title={
                    required
                      ? "Pflichtwert kann nicht entfernt werden"
                      : "Wert entfernen"
                  }
                >
                  {required ? (
                    <LockIcon aria-hidden="true" />
                  ) : (
                    <Trash2Icon aria-hidden="true" />
                  )}
                  <span className="sr-only">
                    {required ? "Pflichtwert geschützt" : "Wert entfernen"}
                  </span>
                </Button>
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}

type ConfigKeyCellProps = {
  entry: SourceConfigEntry;
  index: number;
  keyOptions: ConfigKeyOption[];
  required: boolean;
  disabled: boolean;
  onChange: (key: string) => void;
};

function ConfigKeyCell({
  entry,
  index,
  keyOptions,
  required,
  disabled,
  onChange,
}: ConfigKeyCellProps) {
  const selectedKnownKey = keyOptions.some((option) => option.key === entry.key)
    ? entry.key
    : null;

  return (
    <Combobox
      items={keyOptions.map((option) => option.key)}
      inputValue={entry.key}
      value={selectedKnownKey}
      onInputValueChange={(value) => onChange(value)}
      onValueChange={(value) => {
        if (value) onChange(value);
      }}
      disabled={disabled || required}
      openOnInputClick
    >
      <ComboboxInput
        aria-label={`Key für Konfigurationswert ${index + 1}`}
        placeholder="Key"
        showClear={false}
        className="h-8 rounded-none border-0 bg-transparent shadow-none ring-0 focus-within:ring-0"
        disabled={disabled || required}
      />
      {keyOptions.length ? (
        <ComboboxContent className="min-w-64">
          <ComboboxLabel>Bekannte Schema-Keys</ComboboxLabel>
          <ComboboxSeparator />
          <ComboboxEmpty>Kein Schema-Key gefunden.</ComboboxEmpty>
          <ComboboxList>
            {keyOptions.map((option) => (
              <ComboboxItem key={option.key} value={option.key}>
                <div className="flex min-w-0 flex-col gap-0.5 pr-6">
                  <span className="truncate font-medium">{option.key}</span>
                  <span className="truncate text-muted-foreground">
                    {option.label}
                    {option.required ? " · Pflicht" : ""}
                  </span>
                </div>
              </ComboboxItem>
            ))}
          </ComboboxList>
        </ComboboxContent>
      ) : null}
    </Combobox>
  );
}

type ConfigValueCellProps = {
  entry: SourceConfigEntry;
  index: number;
  propertySchema: JsonObject | undefined;
  disabled: boolean;
  onChange: (value: string) => void;
};

function ConfigValueCell({
  entry,
  index,
  propertySchema,
  disabled,
  onChange,
}: ConfigValueCellProps) {
  const enumOptions = schemaEnumOptions(propertySchema);
  const fieldType = schemaFieldType(propertySchema);
  const ariaLabel = `Wert für ${entry.key || `Konfigurationswert ${index + 1}`}`;

  if (enumOptions.length) {
    return (
      <Select
        items={enumOptions}
        modal={false}
        value={entry.value || null}
        onValueChange={(value) => {
          if (value !== null) onChange(value);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 shadow-none ring-0 focus:ring-0"
          aria-label={ariaLabel}
          disabled={disabled}
        >
          <SelectValue placeholder="Wert wählen" />
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false}>
          <SelectGroup>
            {enumOptions.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    );
  }

  if (fieldType === "boolean") {
    return (
      <Select
        items={booleanOptions}
        modal={false}
        value={normalizedBooleanValue(entry.value)}
        onValueChange={(value) => {
          if (value) onChange(value);
        }}
      >
        <SelectTrigger
          className="h-8 w-full rounded-none border-0 bg-transparent px-2 shadow-none ring-0 focus:ring-0"
          aria-label={ariaLabel}
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

  if (fieldType === "json") {
    const parsedValue = parseJsonValue(entry.value);

    return (
      <div className="grid gap-2 p-0">
        <Textarea
          value={entry.value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="JSON-Wert"
          aria-label={ariaLabel}
          disabled={disabled}
          className="min-h-16 rounded-none border-0 bg-transparent px-2 py-1.5 font-mono shadow-none ring-0 focus-visible:ring-0"
        />
        {parsedValue.ok ? (
          <div className="px-2 pb-2">
            <SchemaValueTable value={parsedValue.value} schema={propertySchema} />
          </div>
        ) : null}
      </div>
    );
  }

  return (
    <Input
      value={entry.value}
      onChange={(event) => onChange(event.target.value)}
      placeholder="Wert"
      aria-label={ariaLabel}
      disabled={disabled}
      type={
        fieldType === "number"
          ? "number"
          : inputTypeForSchema(entry.key, propertySchema)
      }
      className="h-8 rounded-none border-0 bg-transparent shadow-none ring-0 focus-visible:ring-0"
    />
  );
}

type SourceConfigEmptyStateProps = {
  disabled: boolean;
  onAdd: () => void;
};

function SourceConfigEmptyState({
  disabled,
  onAdd,
}: SourceConfigEmptyStateProps) {
  return (
    <Empty className="rounded-md border border-dashed p-4">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <PlusIcon aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>Noch keine Konfigurationswerte</EmptyTitle>
        <EmptyDescription>
          Füge Werte manuell hinzu oder nutze die Linkprüfung, um erkannte Werte
          zu übernehmen.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button
          type="button"
          variant="outline"
          onClick={onAdd}
          disabled={disabled}
        >
          <PlusIcon data-icon="inline-start" aria-hidden="true" />
          Wert hinzufügen
        </Button>
      </EmptyContent>
    </Empty>
  );
}

const booleanOptions = [
  { value: "true", label: "Ja / true" },
  { value: "false", label: "Nein / false" },
];

function schemaTitle(key: string, schema: JsonObject | undefined) {
  return typeof schema?.title === "string" ? schema.title : key;
}

function schemaEnumOptions(schema: JsonObject | undefined) {
  const enumValues = schema?.enum;
  if (!Array.isArray(enumValues)) return [];

  return enumValues
    .filter(isPrimitiveJsonValue)
    .map((value) => ({
      value: jsonValueToInputValue(value),
      label: String(value),
    }));
}

function schemaFieldTypeLabel(type: ReturnType<typeof schemaFieldType>) {
  if (type === "json") return "JSON";
  if (type === "number") return "Zahl";
  if (type === "boolean") return "Boolean";
  return "Text";
}

function inputTypeForSchema(key: string, schema: JsonObject | undefined) {
  if (schema?.format === "uri" || /url$/i.test(key)) return "url";
  return "text";
}

function normalizedBooleanValue(value: string) {
  const normalized = value.trim().toLocaleLowerCase("de");
  if (["true", "1", "ja", "yes"].includes(normalized)) return "true";
  if (["false", "0", "nein", "no"].includes(normalized)) return "false";
  return null;
}

function parseJsonValue(
  value: string,
): { ok: true; value: JsonValue } | { ok: false } {
  try {
    return { ok: true, value: JSON.parse(value) as JsonValue };
  } catch {
    return { ok: false };
  }
}

function isPrimitiveJsonValue(
  value: JsonValue,
): value is string | number | boolean {
  return (
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  );
}
