import { useState } from "react";

import { AlertCircleIcon, ChevronDownIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  diagnosticCodeLabels,
  documentDirectoryLabels,
  documentKindLabels,
  originLabels,
} from "@/features/sources/labels";
import { diagnosticCountLabel } from "@/features/sources/registry-view-model";
import type { SourceRegistryDiagnostic } from "@/lib/api/sources";

type InlineDiagnosticsProps = {
  title: string;
  diagnostics: SourceRegistryDiagnostic[];
};

export function InlineDiagnostics({
  title,
  diagnostics,
}: InlineDiagnosticsProps) {
  const [open, setOpen] = useState(false);

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <Alert variant="warning">
        <AlertCircleIcon className="size-4" aria-hidden="true" />
        <AlertTitle>{title}</AlertTitle>
        <AlertDescription>
          <div className="grid gap-2">
            <p>
              {diagnosticCountLabel(diagnostics.length)} sind diesem Registry
              Key oder Dokumentpfad zugeordnet.
            </p>
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
  diagnostic: SourceRegistryDiagnostic;
};

function DiagnosticSummary({ diagnostic }: DiagnosticSummaryProps) {
  return (
    <div className="grid gap-1 rounded-md border bg-background p-2 text-xs">
      <div className="flex flex-wrap gap-1">
        <Badge variant="warning-light">
          {diagnosticCodeLabels[diagnostic.code]}
        </Badge>
        <Badge variant="outline">
          {documentKindLabels[diagnostic.documentKind]}
        </Badge>
        <Badge variant="outline">{originLabels[diagnostic.origin]}</Badge>
      </div>
      {diagnostic.key ? (
        <p>
          <span className="font-medium">Key:</span>{" "}
          <code>{diagnostic.key}</code>
        </p>
      ) : null}
      <p>{diagnostic.message}</p>
      <p className="break-all font-mono text-muted-foreground">
        {diagnostic.path}
      </p>
    </div>
  );
}

type DiagnosticCardProps = {
  diagnostic: SourceRegistryDiagnostic;
};

export function DiagnosticCard({ diagnostic }: DiagnosticCardProps) {
  return (
    <Card className="border-destructive/40">
      <CardHeader>
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <CardTitle className="text-base">
              {diagnosticCodeLabels[diagnostic.code]}
            </CardTitle>
            <CardDescription>
              {documentKindLabels[diagnostic.documentKind]} ·{" "}
              {originLabels[diagnostic.origin]} ·{" "}
              {documentDirectoryLabels[diagnostic.documentKind]}
            </CardDescription>
          </div>
          <Badge variant="destructive-light">{diagnostic.code}</Badge>
        </div>
      </CardHeader>
      <CardContent className="grid gap-2 text-sm">
        {diagnostic.key ? (
          <p>
            <span className="font-medium">Key:</span>{" "}
            <code>{diagnostic.key}</code>
          </p>
        ) : (
          <p className="text-muted-foreground">Kein Key verfügbar.</p>
        )}
        <p>
          <span className="font-medium">Dokumentart:</span>{" "}
          {documentKindLabels[diagnostic.documentKind]}
        </p>
        <p>
          <span className="font-medium">Ursprung:</span>{" "}
          {originLabels[diagnostic.origin]}
        </p>
        <p className="break-all">
          <span className="font-medium">Pfad:</span> {diagnostic.path}
        </p>
        <p>{diagnostic.message}</p>
      </CardContent>
    </Card>
  );
}
