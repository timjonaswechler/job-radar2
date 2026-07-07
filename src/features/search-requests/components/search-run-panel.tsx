import { AlertCircleIcon, CheckCircle2Icon, RefreshCcwIcon, XCircleIcon } from "lucide-react";

import { Badge, type BadgeProps } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { SourceRunSummary } from "@/features/search-requests/components/source-run-summary";
import type { SearchRequestTableRow } from "@/features/search-requests/model/search-request-row-model";
import { searchRunStatusBadgeVariants, searchRunStatusLabels } from "@/features/search-requests/status";
import type { BackgroundTaskSnapshot, SearchRunResult } from "@/lib/api/search-requests";

const backgroundTaskStateLabels: Record<BackgroundTaskSnapshot["state"], string> = {
  queued: "Wartet",
  running: "Läuft",
  cancelling: "Wird abgebrochen",
  succeeded: "Abgeschlossen",
  failed: "Fehlgeschlagen",
  cancelled: "Abgebrochen",
};

const backgroundTaskStateBadgeVariants: Record<
  BackgroundTaskSnapshot["state"],
  BadgeProps["variant"]
> = {
  queued: "info-light",
  running: "info-light",
  cancelling: "warning-light",
  succeeded: "success-light",
  failed: "destructive-light",
  cancelled: "invert-light",
};

type SearchRunPanelProps = {
  row: SearchRequestTableRow | null;
  starting: boolean;
  task: BackgroundTaskSnapshot | null;
  result: SearchRunResult | null;
  error: string | null;
  cancelling: boolean;
  onCancel: () => void;
};

export function SearchRunPanel({
  row,
  starting,
  task,
  result,
  error,
  cancelling,
  onCancel,
}: SearchRunPanelProps) {
  const inFlight = starting || isInFlightBackgroundTask(task);
  const canCancel = task?.state === "queued" || task?.state === "running";
  const sourceRunCount = result?.sourceRuns.length ?? 0;

  return (
    <Card className="border-info/20" size="sm">
      <CardHeader>
        <CardTitle className="flex flex-wrap items-center gap-2">
          {inFlight ? (
            <RefreshCcwIcon className="size-4 animate-spin text-info" aria-hidden="true" />
          ) : result?.status === "completed" ? (
            <CheckCircle2Icon className="size-4 text-success" aria-hidden="true" />
          ) : result?.status === "failed" || error ? (
            <XCircleIcon className="size-4 text-destructive" aria-hidden="true" />
          ) : (
            <AlertCircleIcon className="size-4 text-muted-foreground" aria-hidden="true" />
          )}
          <span>Search Run</span>
          {task ? (
            <Badge variant={backgroundTaskStateBadgeVariants[task.state]}>
              {backgroundTaskStateLabels[task.state]}
            </Badge>
          ) : null}
          {result ? (
            <Badge variant={searchRunStatusBadgeVariants[result.status]}>
              {searchRunStatusLabels[result.status]}
            </Badge>
          ) : null}
        </CardTitle>
        <CardDescription>
          {row ? `${row.title} · Search Request #${row.id}` : "Search Request wird vorbereitet"}
        </CardDescription>
        {canCancel ? (
          <CardAction>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onCancel}
              disabled={cancelling}
            >
              {cancelling ? "Bricht ab…" : "Abbrechen"}
            </Button>
          </CardAction>
        ) : null}
      </CardHeader>
      <CardContent className="grid gap-3">
        <div className="grid gap-1 text-sm">
          <StatusLine label="Task" value={task ? task.taskId : "Noch kein Task"} />
          <StatusLine
            label="Status"
            value={getPanelStatusLabel(starting, task, result, error)}
          />
          {task?.progress?.message ? (
            <StatusLine label="Fortschritt" value={formatProgress(task)} />
          ) : null}
          {result ? (
            <>
              <StatusLine label="Generiert" value={formatDateTime(result.generatedAt)} />
              <StatusLine
                label="Source Runs"
                value={`${sourceRunCount} Source Run${sourceRunCount === 1 ? "" : "s"}`}
              />
            </>
          ) : null}
        </div>

        {error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/5 p-2 text-sm text-destructive">
            {error}
          </div>
        ) : null}

        {!error && inFlight ? (
          <p className="text-sm text-muted-foreground">
            Der Search Run ist in Arbeit. Ergebnisse und Source-Run Details erscheinen nach Abschluss.
          </p>
        ) : null}

        {!error && task?.state === "cancelled" ? (
          <p className="text-sm text-muted-foreground">
            Der Search Run wurde abgebrochen. Es gibt möglicherweise kein vollständiges Ergebnis.
          </p>
        ) : null}

        {!error && task?.state === "failed" ? (
          <p className="text-sm text-muted-foreground">
            Der Search Run ist fehlgeschlagen. Details stehen im Task-Fehler oder in den Diagnostics.
          </p>
        ) : null}

        {!error && result?.status === "completed" && sourceRunCount === 0 ? (
          <p className="text-sm text-muted-foreground">
            Der Search Run ist abgeschlossen, aber es wurden keine Source Runs zurückgegeben.
          </p>
        ) : null}

        {!error && result?.status === "completed_with_errors" ? (
          <p className="text-sm text-muted-foreground">
            Der Search Run wurde mit Source-Fehlern abgeschlossen. Erfolgreiche Source Runs bleiben erhalten; Details stehen im Source-Run Summary.
          </p>
        ) : null}

        {result ? <SourceRunSummary sourceRuns={result.sourceRuns} /> : null}
      </CardContent>
    </Card>
  );
}

function StatusLine({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1 sm:grid-cols-[8rem_1fr]">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium break-all">{value}</span>
    </div>
  );
}

function getPanelStatusLabel(
  starting: boolean,
  task: BackgroundTaskSnapshot | null,
  result: SearchRunResult | null,
  error: string | null,
) {
  if (starting) return "Startet";
  if (error) return "Fehler";
  if (result) return searchRunStatusLabels[result.status];
  if (task) return backgroundTaskStateLabels[task.state];
  return "Bereit";
}

function formatProgress(task: BackgroundTaskSnapshot) {
  const progress = task.progress;
  if (!progress) return "Kein Fortschritt";
  if (progress.current !== null && progress.total !== null) {
    return `${progress.message} (${progress.current}/${progress.total})`;
  }
  return progress.message;
}

function formatDateTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("de", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function isInFlightBackgroundTask(task: BackgroundTaskSnapshot | null) {
  return task?.state === "queued" || task?.state === "running" || task?.state === "cancelling";
}
