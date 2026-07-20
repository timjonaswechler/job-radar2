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
import { SchemaGuidedValueEditor } from "@/features/sources/schema-editor/schema-editor";
import type { SchemaResolutionOptions } from "@/features/sources/shared/schema-introspection";

const directSourceSpecializationSchemaContext = profileDslSchemaCatalog.resolveRef(
  profileDslSchemaRefs.accessPathFragments,
);
const directSourceSpecializationSchemaOptions: SchemaResolutionOptions = {
  catalog: profileDslSchemaCatalog,
  rootSchema: directSourceSpecializationSchemaContext?.rootSchema,
  baseUri: directSourceSpecializationSchemaContext?.baseUri,
};

type DirectSourceSpecializationEditorProps = {
  value: string;
  disabled: boolean;
  starterValue: string;
  errors: string[];
  showErrors: boolean;
  onChange: (value: string) => void;
};

export function DirectSourceSpecializationEditor({
  value,
  disabled,
  starterValue,
  errors,
  showErrors,
  onChange,
}: DirectSourceSpecializationEditorProps) {
  const editing = Boolean(value.trim());
  const invalid = showErrors && errors.length > 0;

  return (
    <FieldSet>
      <FieldLegend>Direkte Source-Spezialisierung</FieldLegend>
      <FieldDescription>
        Optionale, kontrollierte Verhaltensänderungen für den gewählten
        Profil-Zugriffspfad. Source Config bleibt davon getrennt; die
        Backend-Schema- und Compiler-Validierung bleibt maßgeblich.
      </FieldDescription>
      <FieldGroup>
        {editing ? (
          <Field data-invalid={invalid || undefined}>
            <div className="flex items-center justify-between gap-2">
              <FieldLabel>Access-Path-Fragmente</FieldLabel>
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
              ariaLabel="Direkte Source-Spezialisierung JSON"
              schema={directSourceSpecializationSchemaContext?.schema}
              schemaOptions={directSourceSpecializationSchemaOptions}
              disabled={disabled}
              previewMaxDepth={4}
              textareaClassName="min-h-40 font-mono"
            />
            <FieldDescription>
              Nutze <code>accessPaths</code>, um einzelne Strategien bei
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
            <FieldLabel className="sr-only">Direkte Source-Spezialisierung</FieldLabel>
            <div className="rounded-md border border-dashed p-4">
              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-sm text-muted-foreground">
                  Keine direkte Source-Spezialisierung gesetzt. Das Profilverhalten wird unverändert geerbt.
                </p>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => onChange(starterValue)}
                  disabled={disabled}
                >
                  <Code2Icon data-icon="inline-start" aria-hidden="true" />
                  Fragment-Vorlage einfügen
                </Button>
              </div>
            </div>
          </Field>
        )}
      </FieldGroup>
    </FieldSet>
  );
}
