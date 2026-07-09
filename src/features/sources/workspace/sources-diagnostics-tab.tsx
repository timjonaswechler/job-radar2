import { AlertCircleIcon, CheckCircle2Icon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { DiagnosticCard } from "@/features/sources/registry/diagnostics/diagnostic-card";
import { diagnosticCountLabel } from "@/features/sources/view-model/diagnostics";
import type { StructuredDiagnostic } from "@/lib/api/sources";

type SourcesDiagnosticsTabProps = {
  diagnostics: StructuredDiagnostic[];
  unassignedDiagnostics: StructuredDiagnostic[];
  loading: boolean;
};

export function SourcesDiagnosticsTab({
  diagnostics,
  unassignedDiagnostics,
  loading,
}: SourcesDiagnosticsTabProps) {
  if (loading) return <DiagnosticsSkeleton />;

  if (!diagnostics.length) {
    return (
      <Alert variant="success">
        <CheckCircle2Icon className="size-4" aria-hidden="true" />
        <AlertTitle>Keine Registry-Diagnosen</AlertTitle>
        <AlertDescription>
          Alle geladenen Source-Registry-Dokumente sind gültig und ihre
          Profil-/Zugriffspfad-Referenzen konnten aufgelöst werden.
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <>
      {unassignedDiagnostics.length ? (
        <Alert variant="warning">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>
            {diagnosticCountLabel(unassignedDiagnostics.length)} ohne gültige
            Source-/Profil-Zeile
          </AlertTitle>
          <AlertDescription>
            Diese Diagnosen gehören zu Registry-Dokumenten, die nicht als
            gültige Quelle oder gültiges Profil geladen wurden. Sie bleiben
            hier global sichtbar.
          </AlertDescription>
        </Alert>
      ) : null}
      <div className="grid gap-3 md:grid-cols-2">
        {diagnostics.map((diagnostic, index) => (
          <div
            key={`${diagnostic.path}-${diagnostic.code}-${index}`}
            className="[contain-intrinsic-size:220px] [content-visibility:auto]"
          >
            <DiagnosticCard diagnostic={diagnostic} />
          </div>
        ))}
      </div>
    </>
  );
}

function DiagnosticsSkeleton() {
  return (
    <div className="grid gap-3 md:grid-cols-2">
      {Array.from({ length: 4 }).map((_, index) => (
        <Card key={index}>
          <CardHeader>
            <Skeleton className="h-5 w-1/2" />
            <Skeleton className="h-4 w-2/3" />
          </CardHeader>
          <CardContent className="grid gap-2">
            <Skeleton className="h-4 w-1/3" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-5/6" />
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
