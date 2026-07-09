import { useCallback, useEffect, useMemo, useState } from "react";

import {
  AlertCircleIcon,
  CheckCircle2Icon,
  Loader2Icon,
  RefreshCwIcon,
} from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { InlineDiagnostics } from "@/features/sources/registry/diagnostics/inline-diagnostics";
import { sourceStatusLabels } from "@/features/sources/status";
import {
  sourceLiveCheckActionsForSource,
  sourceLiveCheckDisplayModel,
  type SourceLiveCheckActionKind,
} from "@/features/sources/view-model/source-live-check-model";
import {
  checkAndActivateSource,
  checkAndReactivateSource,
  checkSource,
  getSourceLiveCheckReportStatus,
  type CheckReport,
  type RegistrySource,
  type SourceLiveCheckReportStatus,
} from "@/lib/api/sources";

type SourceLiveCheckSectionProps = {
  source: RegistrySource;
  onUpdated?: () => Promise<unknown> | unknown;
};

export function SourceLiveCheckSection({
  source,
  onUpdated,
}: SourceLiveCheckSectionProps) {
  const sourceKey = source.document.key;
  const [liveCheckStatus, setLiveCheckStatus] =
    useState<SourceLiveCheckReportStatus | null>(null);
  const [liveCheckLoading, setLiveCheckLoading] = useState(true);
  const [runningAction, setRunningAction] =
    useState<SourceLiveCheckActionKind | null>(null);
  const [liveCheckError, setLiveCheckError] = useState<string | null>(null);
  const model = sourceLiveCheckDisplayModel(liveCheckStatus);
  const actions = useMemo(
    () => sourceLiveCheckActionsForSource(source.document.status),
    [source.document.status],
  );
  const alertVariant =
    model.displayState === "passed"
      ? "success"
      : model.displayState === "failed"
        ? "destructive"
        : model.displayState === "stale"
          ? "warning"
          : "info";
  const StatusIcon = model.displayState === "passed" ? CheckCircle2Icon : AlertCircleIcon;

  const refreshLiveCheckStatus = useCallback(async () => {
    setLiveCheckLoading(true);
    setLiveCheckError(null);
    try {
      setLiveCheckStatus(await getSourceLiveCheckReportStatus(sourceKey));
    } catch (unknownError) {
      setLiveCheckStatus(null);
      setLiveCheckError(errorMessage(unknownError));
    } finally {
      setLiveCheckLoading(false);
    }
  }, [sourceKey]);

  useEffect(() => {
    void refreshLiveCheckStatus();
  }, [refreshLiveCheckStatus]);

  const runLiveCheckAction = useCallback(
    async (kind: SourceLiveCheckActionKind) => {
      const action = actions.find((item) => item.kind === kind);
      setRunningAction(kind);
      setLiveCheckError(null);
      try {
        let report: CheckReport;
        if (kind === "check") {
          report = await checkSource(sourceKey);
        } else if (kind === "check_and_activate") {
          report = await checkAndActivateSource(sourceKey);
        } else {
          report = await checkAndReactivateSource(sourceKey);
        }
        const nextStatus = await getSourceLiveCheckReportStatus(sourceKey);
        setLiveCheckStatus(nextStatus);
        if (kind !== "check" && report.result === "passed") await onUpdated?.();
        if (report.result === "passed") {
          toast.success(action?.label ?? "Source Live Check ausgeführt", {
            description:
              kind === "check"
                ? `Source ${sourceKey} wurde status-neutral geprüft.`
                : `Source ${sourceKey} wurde nach bestandener Live-Prüfung aktualisiert.`,
          });
        } else {
          toast.warning(
            kind === "check"
              ? "Source Live Check fehlgeschlagen"
              : "Aktivierung wurde blockiert",
            {
              description:
                kind === "check"
                  ? `Source ${sourceKey} bleibt im aktuellen Status.`
                  : `Source ${sourceKey} wurde nicht aktiviert. Diagnosen im Report prüfen.`,
            },
          );
        }
      } catch (unknownError) {
        const message = errorMessage(unknownError);
        setLiveCheckError(message);
        try {
          setLiveCheckStatus(await getSourceLiveCheckReportStatus(sourceKey));
        } catch {
          // Keep the original action error visible when report refresh also fails.
        }
        toast.error(action?.label ?? "Source Live Check fehlgeschlagen", {
          description: message,
        });
      } finally {
        setRunningAction(null);
      }
    },
    [actions, onUpdated, sourceKey],
  );

  return (
    <section className="grid gap-3 rounded-lg border bg-muted/30 p-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid gap-1">
          <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Source Live Check
          </h3>
          <p className="text-xs text-muted-foreground">
            Live-/Source-spezifische Prüfung. Source Status und
            Live-Check-Zustand bleiben getrennt; Prüfen allein ändert den Status
            nicht.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={liveCheckLoading || runningAction !== null}
            onClick={refreshLiveCheckStatus}
          >
            <RefreshCwIcon data-icon="inline-start" aria-hidden="true" />
            Status laden
          </Button>
          {actions.map((action) => (
            <Button
              key={action.kind}
              type="button"
              size="sm"
              variant={action.kind === "check" ? "outline" : "default"}
              title={action.description}
              disabled={liveCheckLoading || runningAction !== null}
              onClick={() => void runLiveCheckAction(action.kind)}
            >
              {runningAction === action.kind ? (
                <Loader2Icon
                  data-icon="inline-start"
                  className="animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <CheckCircle2Icon data-icon="inline-start" aria-hidden="true" />
              )}
              {action.label}
            </Button>
          ))}
        </div>
      </div>

      {liveCheckLoading ? (
        <Alert variant="info">
          <Loader2Icon className="size-4 animate-spin" aria-hidden="true" />
          <AlertTitle>Source Live Check Report wird geladen</AlertTitle>
          <AlertDescription>Der neueste persistierte Report wird gelesen.</AlertDescription>
        </Alert>
      ) : null}

      {liveCheckError ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Source Live Check konnte nicht ausgeführt werden</AlertTitle>
          <AlertDescription>{liveCheckError}</AlertDescription>
        </Alert>
      ) : null}

      {!liveCheckLoading ? (
        <Alert variant={alertVariant}>
          <StatusIcon className="size-4" aria-hidden="true" />
          <AlertTitle>{model.displayLabel}</AlertTitle>
          <AlertDescription>
            {model.displayState === "unknown"
              ? "Es gibt noch keinen Source Live Check Report für diese Source."
              : model.displayState === "stale"
                ? "Der neueste Report passt nicht mehr zu den aktuellen Source-Eingaben."
                : "Der angezeigte Zustand stammt aus dem neuesten Source Live Check Report."}
          </AlertDescription>
        </Alert>
      ) : null}

      <div className="flex flex-wrap gap-1.5">
        <Badge variant="outline">
          Source Status: {sourceStatusLabels[source.document.status]}
        </Badge>
        <Badge variant={sourceLiveCheckBadgeVariant(model.displayState)}>
          Live Check: {model.displayLabel}
        </Badge>
      </div>

      <dl className="grid gap-3 sm:grid-cols-2">
        <DetailRow
          label="Source Status"
          value={sourceStatusLabels[source.document.status]}
        />
        <DetailRow label="Live Check" value={model.displayLabel} />
        <DetailRow label="Report-Zustand" value={model.reportStateLabel} />
        <DetailRow label="Report Result" value={model.reportResultLabel} />
        <DetailRow
          label="Checked At"
          value={
            liveCheckStatus?.report?.checkedAt
              ? new Date(liveCheckStatus.report.checkedAt).toLocaleString("de")
              : "—"
          }
        />
        <DetailRow
          label="Access Path"
          value={String(liveCheckStatus?.report?.details.accessPathKey ?? "—")}
          mono
        />
      </dl>

      {model.staleFingerprints.length ? (
        <div className="grid gap-2">
          <h4 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Stale-Fingerprints
          </h4>
          <div className="grid gap-2">
            {model.staleFingerprints.map((stale, index) => (
              <div
                key={`${stale.kind}-${stale.reference ?? "none"}-${index}`}
                className="grid gap-1 rounded-md border bg-background p-2 text-xs"
              >
                <div className="flex flex-wrap gap-1">
                  <Badge variant="warning-light">{stale.reason}</Badge>
                  <Badge variant="outline">{stale.kind}</Badge>
                  {stale.reference ? <Badge variant="outline">{stale.reference}</Badge> : null}
                </div>
                {stale.expectedValue || stale.actualValue ? (
                  <p className="break-all text-muted-foreground">
                    Erwartet {stale.expectedValue ?? stale.expectedSha256 ?? "—"}, Report {stale.actualValue ?? stale.actualSha256 ?? "—"}
                  </p>
                ) : null}
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {model.diagnostics.length ? (
        <InlineDiagnostics
          title="Source-Live-Check-Diagnosen"
          diagnostics={model.diagnostics}
        />
      ) : null}
    </section>
  );
}

function sourceLiveCheckBadgeVariant(
  state: ReturnType<typeof sourceLiveCheckDisplayModel>["displayState"],
) {
  if (state === "passed") return "success-light";
  if (state === "failed") return "destructive-light";
  if (state === "stale") return "warning-light";
  return "secondary";
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
