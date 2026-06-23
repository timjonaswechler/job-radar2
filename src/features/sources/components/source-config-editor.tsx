import { PlusIcon, Trash2Icon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
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
import { createEntryId } from "@/features/sources/source-add-model";
import {
  configEntryDescription,
  jsonValueToInputValue,
  schemaDefaultValue,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/source-config-schema";

type SourceConfigEditorProps = {
  entries: SourceConfigEntry[];
  schemaMetadata: SchemaMetadata;
  disabled: boolean;
  configErrors: string[];
  showErrors: boolean;
  onChange: (entries: SourceConfigEntry[]) => void;
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
        Werte werden als Key/Value-Paare gepflegt und beim Speichern in <code>sourceConfig</code>
        geschrieben.
      </FieldDescription>
      {knownKeys.length ? (
        <div className="flex flex-wrap gap-1">
          {knownKeys.map((key) => (
            <Badge
              key={key}
              variant={schemaMetadata.requiredKeys.has(key) ? "warning-light" : "outline"}
            >
              {key}
              {schemaMetadata.requiredKeys.has(key) ? " · Pflicht" : ""}
            </Badge>
          ))}
        </div>
      ) : null}
      <FieldGroup>
        {entries.length ? (
          entries.map((entry, index) => {
            const propertySchema = schemaMetadata.properties.get(entry.key);
            const required = schemaMetadata.requiredKeys.has(entry.key);
            return (
              <Field key={entry.id}>
                <FieldLabel className="sr-only">Konfigurationswert {index + 1}</FieldLabel>
                <div className="grid gap-2 sm:grid-cols-[minmax(0,0.75fr)_minmax(0,1fr)_auto]">
                  <Input
                    value={entry.key}
                    onChange={(event) => updateEntry(entry.id, { key: event.target.value })}
                    placeholder="Key"
                    aria-label={`Key für Konfigurationswert ${index + 1}`}
                    disabled={disabled}
                  />
                  <Input
                    value={entry.value}
                    onChange={(event) => updateEntry(entry.id, { value: event.target.value })}
                    placeholder="Wert"
                    aria-label={`Wert für ${entry.key || `Konfigurationswert ${index + 1}`}`}
                    disabled={disabled}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={() => removeEntry(entry.id)}
                    disabled={disabled || required}
                    title={required ? "Pflichtwert kann nicht entfernt werden" : "Wert entfernen"}
                  >
                    <Trash2Icon aria-hidden="true" />
                    <span className="sr-only">Wert entfernen</span>
                  </Button>
                </div>
                <FieldDescription>
                  {entry.key ? configEntryDescription(entry.key, propertySchema, required) : "Freier Konfigurationswert."}
                </FieldDescription>
              </Field>
            );
          })
        ) : (
          <Alert>
            <PlusIcon aria-hidden="true" />
            <AlertTitle>Noch keine Konfigurationswerte</AlertTitle>
            <AlertDescription>
              Füge Werte manuell hinzu oder nutze die Linkprüfung, um erkannte Werte zu übernehmen.
            </AlertDescription>
          </Alert>
        )}
        <Button type="button" variant="outline" onClick={addEntry} disabled={disabled}>
          <PlusIcon data-icon="inline-start" aria-hidden="true" />
          Wert hinzufügen
        </Button>
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
