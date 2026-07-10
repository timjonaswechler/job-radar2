import { PlusIcon } from "lucide-react";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
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
import {
  createSourceConfigEntryId,
  jsonValueToInputValue,
  schemaDefaultValue,
  type JsonObject,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";

import type { ConfigKeyOption } from "./config-key-control";
import { SourceConfigTable } from "./source-config-table";

type SourceConfigEditorProps = {
  entries: SourceConfigEntry[];
  schemaMetadata: SchemaMetadata;
  disabled: boolean;
  configErrors: string[];
  showErrors: boolean;
  portalContainer?: HTMLElement | null;
  onChange: (entries: SourceConfigEntry[]) => void;
};

export function SourceConfigEditor({
  entries,
  schemaMetadata,
  disabled,
  configErrors,
  showErrors,
  portalContainer,
  onChange,
}: SourceConfigEditorProps) {
  const knownKeys = [...schemaMetadata.properties.keys()];
  const keyOptions = knownKeys.map((key): ConfigKeyOption => {
    const schema = schemaMetadata.properties.get(key);
    return {
      key,
      label: schemaTitle(key, schema),
      description:
        typeof schema?.description === "string" ? schema.description : undefined,
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
        id: createSourceConfigEntryId(),
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
              portalContainer={portalContainer}
              onUpdate={updateEntry}
              onRemove={removeEntry}
            />
            <FieldDescription>
              Pflichtwerte stammen aus dem effektiven Profil-/Access-Path-Schema.
              Bereits gespeicherte Pflichtwerte sind geschützt; neu hinzugefügte
              Pflichtwerte bleiben bis zum Speichern entfernbar.
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

function SchemaKeyLegend({ options }: { options: ConfigKeyOption[] }) {
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

function SourceConfigEmptyState({
  disabled,
  onAdd,
}: {
  disabled: boolean;
  onAdd: () => void;
}) {
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

function schemaTitle(key: string, schema: JsonObject | undefined) {
  return typeof schema?.title === "string" ? schema.title : key;
}
