import { Badge } from "@/components/reui/badge";
import type { SchemaGuidedValueEditorModel } from "@/features/sources/schema-editor/schema-editor-model";

export function SchemaGuidance({
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
          {model.schemaTitle ? (
            <p className="font-medium">{model.schemaTitle}</p>
          ) : null}
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

export function SchemaGuidedEditError({ message }: { message: string }) {
  return (
    <p role="alert" className="text-xs text-destructive">
      {message}
    </p>
  );
}
