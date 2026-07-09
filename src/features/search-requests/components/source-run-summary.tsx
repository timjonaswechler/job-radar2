import { AlertCircleIcon } from "lucide-react";

import { Badge, type BadgeProps } from "@/components/reui/badge";
import { diagnosticSeverityBadgeVariants } from "@/features/search-requests/components/diagnostic-severity-badges";
import type { SourceRunResult, SourceRunStatus } from "@/lib/api/search-requests";
import type { StructuredDiagnostic } from "@/lib/api/sources";

const sourceRunStatusLabels: Record<SourceRunStatus, string> = {
  completed: "Abgeschlossen",
  failed: "Fehlgeschlagen",
  cancelled: "Abgebrochen",
  skipped: "Übersprungen",
};

const sourceRunStatusBadgeVariants: Record<SourceRunStatus, BadgeProps["variant"]> = {
  completed: "success-light",
  failed: "destructive-light",
  cancelled: "invert-light",
  skipped: "warning-light",
};

type SourceRunSummaryProps = {
  sourceRuns: SourceRunResult[];
};

export function SourceRunSummary({ sourceRuns }: SourceRunSummaryProps) {
  if (!sourceRuns.length) {
    return (
      <div className="rounded-md border border-dashed p-3 text-sm text-muted-foreground">
        Keine Source Runs im Search Run-Ergebnis.
      </div>
    );
  }

  return (
    <section className="grid gap-2" aria-labelledby="source-run-summary-title">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 id="source-run-summary-title" className="text-sm font-medium">
          Source Runs
        </h2>
        <span className="text-xs text-muted-foreground">
          {sourceRuns.length} Source Run{sourceRuns.length === 1 ? "" : "s"}
        </span>
      </div>
      <div className="grid gap-2">
        {sourceRuns.map((sourceRun, index) => (
          <SourceRunSummaryItem
            key={`${sourceRun.sourceKey}-${index}`}
            sourceRun={sourceRun}
          />
        ))}
      </div>
    </section>
  );
}

function SourceRunSummaryItem({ sourceRun }: { sourceRun: SourceRunResult }) {
  const primaryDiagnostic = firstImportantDiagnostic(sourceRun.diagnostics);

  return (
    <article className="grid gap-2 rounded-md border bg-background p-3">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="grid gap-0.5">
          <div className="font-medium">{sourceRun.sourceName}</div>
          <div className="text-xs text-muted-foreground">{sourceRun.sourceKey}</div>
        </div>
        <Badge variant={sourceRunStatusBadgeVariants[sourceRun.status]}>
          {sourceRunStatusLabels[sourceRun.status]}
        </Badge>
      </div>

      <dl className="grid gap-2 text-sm sm:grid-cols-2">
        <Metric label="Kandidaten" value={sourceRun.candidateCount} />
        <Metric label="Treffer" value={sourceRun.matchedCount} />
      </dl>

      {sourceRun.error ? (
        <div className="flex gap-2 rounded-md border border-destructive/30 bg-destructive/5 p-2 text-sm text-destructive">
          <AlertCircleIcon className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
          <span>{sourceRun.error}</span>
        </div>
      ) : null}

      {primaryDiagnostic ? (
        <div className="grid gap-1 rounded-md border bg-muted/30 p-2 text-sm">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge variant={diagnosticSeverityBadgeVariants[primaryDiagnostic.severity]}>
              {primaryDiagnostic.severity}
            </Badge>
            <span className="font-medium">{primaryDiagnostic.code}</span>
            {sourceRun.diagnostics.length > 1 ? (
              <span className="text-xs text-muted-foreground">
                +{sourceRun.diagnostics.length - 1} weitere Diagnostic{sourceRun.diagnostics.length === 2 ? "" : "s"}
              </span>
            ) : null}
          </div>
          <p className="text-muted-foreground">{primaryDiagnostic.message}</p>
        </div>
      ) : sourceRun.diagnostics.length ? (
        <div className="text-xs text-muted-foreground">
          {sourceRun.diagnostics.length} Diagnostic{sourceRun.diagnostics.length === 1 ? "" : "s"}
        </div>
      ) : null}
    </article>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-md bg-muted/40 px-2 py-1.5">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="font-medium">{value}</dd>
    </div>
  );
}

function firstImportantDiagnostic(diagnostics: StructuredDiagnostic[]) {
  return (
    diagnostics.find((diagnostic) => diagnostic.severity === "error") ??
    diagnostics.find((diagnostic) => diagnostic.severity === "warning") ??
    diagnostics[0] ??
    null
  );
}
