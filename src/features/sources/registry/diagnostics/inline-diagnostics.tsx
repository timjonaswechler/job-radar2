import { useState } from "react";

import { AlertCircleIcon, ChevronDownIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { diagnosticCodeLabel, documentKindLabels, originLabels } from "@/features/sources/labels";
import {
  diagnosticCountLabel,
  diagnosticDocumentKey,
  diagnosticDocumentKind,
  diagnosticDocumentOrigin,
  diagnosticDocumentPath,
} from "@/features/sources/view-model/diagnostics";
import type { StructuredDiagnostic } from "@/lib/api/sources";

type InlineDiagnosticsProps = {
  title: string;
  diagnostics: StructuredDiagnostic[];
};

export function InlineDiagnostics({ title, diagnostics }: InlineDiagnosticsProps) {
  const [open, setOpen] = useState(false);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <Alert variant="warning">
        <AlertCircleIcon className="size-4" aria-hidden="true" />
        <AlertTitle>{title}</AlertTitle>
        <AlertDescription>
          <div className="grid gap-2">
            <p>{diagnosticCountLabel(diagnostics.length)} sind zugeordnet.</p>
            <CollapsibleTrigger
              render={
                <Button
                  type="button"
                  variant="outline"
                  size="xs"
                  className="group"
                />
              }
            >
              <ChevronDownIcon
                data-icon="inline-start"
                className="transition-transform group-data-[state=open]:rotate-180"
                aria-hidden="true"
              />
              {open ? "Diagnosen ausblenden" : "Diagnosen anzeigen"}
            </CollapsibleTrigger>
            <CollapsibleContent className="grid gap-2">
              {diagnostics.map((diagnostic, index) => (
                <DiagnosticSummary
                  key={`${diagnostic.path}-${diagnostic.code}-${index}`}
                  diagnostic={diagnostic}
                />
              ))}
            </CollapsibleContent>
          </div>
        </AlertDescription>
      </Alert>
    </Collapsible>
  );
}

type DiagnosticSummaryProps = {
  diagnostic: StructuredDiagnostic;
};

function DiagnosticSummary({ diagnostic }: DiagnosticSummaryProps) {
  const documentKind = diagnosticDocumentKind(diagnostic);
  const origin = diagnosticDocumentOrigin(diagnostic);
  const documentKey = diagnosticDocumentKey(diagnostic);
  const documentPath = diagnosticDocumentPath(diagnostic);

  return (
    <div className="grid gap-1 rounded-md border bg-background p-2 text-xs">
      <div className="flex flex-wrap gap-1">
        <Badge variant={diagnostic.severity === "error" ? "warning-light" : "outline"}>
          {diagnosticCodeLabel(diagnostic.code)}
        </Badge>
        <Badge variant="outline">{diagnostic.category}</Badge>
        <Badge variant={diagnostic.severity === "error" ? "destructive-light" : "outline"}>
          {diagnostic.severity}
        </Badge>
        {documentKind ? (
          <Badge variant="outline">{documentKindLabels[documentKind]}</Badge>
        ) : null}
        {origin ? <Badge variant="outline">{originLabels[origin]}</Badge> : null}
      </div>
      {documentKey ? (
        <p>
          <span className="font-medium">Key:</span> <code>{documentKey}</code>
        </p>
      ) : null}
      <p>{diagnostic.message}</p>
      <p className="break-all font-mono text-muted-foreground">
        {documentPath ?? diagnostic.path}
      </p>
      {diagnostic.details ? (
        <pre className="max-h-32 overflow-auto rounded bg-muted p-2 font-mono text-[0.7rem] text-muted-foreground">
          {JSON.stringify(diagnostic.details, null, 2)}
        </pre>
      ) : null}
    </div>
  );
}
