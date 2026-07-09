import { Badge } from "@/components/reui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  diagnosticCodeLabel,
  documentDirectoryLabels,
  documentKindLabels,
  originLabels,
} from "@/features/sources/labels";
import {
  diagnosticDocumentKey,
  diagnosticDocumentKind,
  diagnosticDocumentOrigin,
  diagnosticDocumentPath,
} from "@/features/sources/view-model/diagnostics";
import type { StructuredDiagnostic } from "@/lib/api/sources";

type DiagnosticCardProps = {
  diagnostic: StructuredDiagnostic;
};

export function DiagnosticCard({ diagnostic }: DiagnosticCardProps) {
  const documentKind = diagnosticDocumentKind(diagnostic);
  const origin = diagnosticDocumentOrigin(diagnostic);
  const documentKey = diagnosticDocumentKey(diagnostic);
  const documentPath = diagnosticDocumentPath(diagnostic);

  return (
    <Card className="border-destructive/40">
      <CardHeader>
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <CardTitle className="text-base">
              {diagnosticCodeLabel(diagnostic.code)}
            </CardTitle>
            <CardDescription>
              {documentKind ? documentKindLabels[documentKind] : diagnostic.category}
              {origin ? ` · ${originLabels[origin]}` : ""}
              {documentKind ? ` · ${documentDirectoryLabels[documentKind]}` : ""}
            </CardDescription>
          </div>
          <Badge variant="destructive-light">{diagnostic.code}</Badge>
        </div>
      </CardHeader>
      <CardContent className="grid gap-2 text-sm">
        {documentKey ? (
          <p>
            <span className="font-medium">Key:</span> <code>{documentKey}</code>
          </p>
        ) : (
          <p className="text-muted-foreground">Kein Registry-Key verfügbar.</p>
        )}
        <p>
          <span className="font-medium">Kategorie:</span> {diagnostic.category}
        </p>
        <p>
          <span className="font-medium">Schweregrad:</span> {diagnostic.severity}
        </p>
        <p className="break-all">
          <span className="font-medium">Pfad:</span> {documentPath ?? diagnostic.path}
        </p>
        {diagnostic.strategyKey ? (
          <p>
            <span className="font-medium">Strategie:</span>{" "}
            <code>{diagnostic.strategyKey}</code>
          </p>
        ) : null}
        <p>{diagnostic.message}</p>
        {diagnostic.details ? (
          <pre className="max-h-48 overflow-auto rounded bg-muted p-2 font-mono text-xs text-muted-foreground">
            {JSON.stringify(diagnostic.details, null, 2)}
          </pre>
        ) : null}
      </CardContent>
    </Card>
  );
}
