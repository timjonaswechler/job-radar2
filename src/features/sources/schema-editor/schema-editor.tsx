import { Textarea } from "@/components/ui/textarea";
import { ArrayEditor } from "@/features/sources/schema-editor/array-editor";
import { ObjectEditor } from "@/features/sources/schema-editor/object-editor";
import { SchemaGuidance } from "@/features/sources/schema-editor/schema-guidance";
import { createSchemaGuidedValueEditorModel } from "@/features/sources/schema-editor/schema-editor-model";
import type {
  JsonObject,
  SchemaResolutionOptions,
} from "@/features/sources/shared/schema-introspection";
import { SchemaValueTable } from "@/features/sources/shared/schema-value-table";

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
        <SchemaGuidance model={model} />
        {model.parseState.ok ? (
          <>
            <ObjectEditor
              model={model}
              schema={schema}
              schemaOptions={schemaOptions}
              disabled={disabled}
              onChange={onChange}
            />
            <ArrayEditor
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
