import { Code2Icon, Trash2Icon } from "lucide-react";

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
import {
  profileDslSchemaCatalog,
  profileDslSchemaRefs,
} from "@/features/sources/shared/profile-dsl-schema-catalog";
import { SchemaGuidedValueEditor } from "@/features/sources/shared/schema-guided-value-editor";
import type { SchemaResolutionOptions } from "@/features/sources/shared/schema-introspection";

const sourceOverridesSchemaContext = profileDslSchemaCatalog.resolveRef(
  profileDslSchemaRefs.sourceOverrides,
);
const sourceOverridesSchemaOptions: SchemaResolutionOptions = {
  catalog: profileDslSchemaCatalog,
  rootSchema: sourceOverridesSchemaContext?.rootSchema,
  baseUri: sourceOverridesSchemaContext?.baseUri,
};

type SourceOverridesEditorProps = {
  value: string;
  disabled: boolean;
  starterValue: string;
  errors: string[];
  showErrors: boolean;
  onChange: (value: string) => void;
};

export function SourceOverridesEditor({
  value,
  disabled,
  starterValue,
  errors,
  showErrors,
  onChange,
}: SourceOverridesEditorProps) {
  const editing = Boolean(value.trim());
  const invalid = showErrors && errors.length > 0;

  return (
    <FieldSet>
      <FieldLegend>Source Overrides</FieldLegend>
      <FieldDescription>
        Optionale, kontrollierte Verhaltensänderungen für den gewählten
        Profil-Zugriffspfad. Source Config bleibt davon getrennt; die
        Backend-Schema- und Compiler-Validierung bleibt maßgeblich.
      </FieldDescription>
      <FieldGroup>
        {editing ? (
          <Field data-invalid={invalid || undefined}>
            <div className="flex items-center justify-between gap-2">
              <FieldLabel>Overrides JSON</FieldLabel>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => onChange("")}
                disabled={disabled}
              >
                <Trash2Icon data-icon="inline-start" aria-hidden="true" />
                Entfernen
              </Button>
            </div>
            <SchemaGuidedValueEditor
              value={value}
              onChange={onChange}
              ariaLabel="Source Overrides JSON"
              schema={sourceOverridesSchemaContext?.schema}
              schemaOptions={sourceOverridesSchemaOptions}
              disabled={disabled}
              previewMaxDepth={4}
              textareaClassName="min-h-40 font-mono"
            />
            <FieldDescription>
              Nutze <code>strategyOverrides</code>, um einzelne Strategien bei
              Bedarf schema-geführt zu überschreiben.
            </FieldDescription>
            {invalid ? (
              <FieldError>
                <ul className="list-inside list-disc">
                  {errors.map((error) => (
                    <li key={error}>{error}</li>
                  ))}
                </ul>
              </FieldError>
            ) : null}
          </Field>
        ) : (
          <Field>
            <FieldLabel className="sr-only">Source Overrides</FieldLabel>
            <div className="rounded-md border border-dashed p-4">
              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-sm text-muted-foreground">
                  Keine Source Overrides gesetzt. Für normale Sources bleibt das
                  Feld leer.
                </p>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => onChange(starterValue)}
                  disabled={disabled}
                >
                  <Code2Icon data-icon="inline-start" aria-hidden="true" />
                  Override-Vorlage einfügen
                </Button>
              </div>
            </div>
          </Field>
        )}
      </FieldGroup>
    </FieldSet>
  );
}
