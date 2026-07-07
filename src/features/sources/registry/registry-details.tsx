import { useCallback, useEffect, useMemo, useState } from "react";

import {
  AlertCircleIcon,
  CheckCircle2Icon,
  Loader2Icon,
  PencilIcon,
  RefreshCwIcon,
  XIcon,
} from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";

import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import {
  AccessPathDetails,
  ProfileAccessPathRow,
} from "@/features/sources/registry/access-path-details";
import { DetailRow } from "@/features/sources/registry/detail-row";
import { OptionalJsonPreview } from "@/features/sources/shared/json-preview";
import {
  OptionalSchemaValuePreview,
  SchemaValuePreview,
} from "@/features/sources/shared/schema-value-table";
import { profileDslSchemaRefs } from "@/features/sources/shared/profile-dsl-schema-catalog";
import { InlineDiagnostics } from "@/features/sources/registry/registry-diagnostics";
import {
  checkReportResultLabels,
  detectionEvidenceKindLabels,
  originLabels,
  profileKindLabels,
  supportEvidenceKindLabels,
  supportLevelLabels,
  validationStateLabels,
} from "@/features/sources/labels";
import {
  profileVerificationDisplayModel,
  resolveSource,
  sourceLiveCheckActionsForSource,
  sourceLiveCheckDisplayModel,
  type ProfileGridRow,
  type SourceGridRow,
  type SourceLiveCheckActionKind,
} from "@/features/sources/view-model/registry-view-model";
import { sourceStatusLabels } from "@/features/sources/status";
import {
  checkAndActivateSource,
  checkAndReactivateSource,
  checkSource,
  getSourceLiveCheckReportStatus,
  getSourceProfileVerificationReportStatus,
  verifySourceProfile,
} from "@/lib/api/sources";
import type {
  CheckReport,
  FixtureCheckResult,
  RegistrySource,
  RegistrySourceProfile,
  SourceLiveCheckReportStatus,
  SourceProfileVerificationReportStatus,
  StructuredDiagnostic,
} from "@/lib/api/sources";

type SourceDetailsDrawerProps = {
  row: SourceGridRow | null;
  profilesByKey: Map<string, RegistrySourceProfile>;
  diagnostics: StructuredDiagnostic[];
  open: boolean;
  onEdit?: (source: RegistrySource) => void;
  onUpdated?: () => Promise<unknown> | unknown;
  onOpenChange: (open: boolean) => void;
};

export function SourceDetailsDrawer({
  row,
  profilesByKey,
  diagnostics,
  open,
  onEdit,
  onUpdated,
  onOpenChange,
}: SourceDetailsDrawerProps) {
  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      {row ? (
        <DrawerContent
          className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw-115px),960px)]
        data-[vaul-drawer-direction=right]:sm:max-w-none"
        >
          <DrawerHeader className="border-b pr-12">
            <DrawerTitle>{row.name}</DrawerTitle>
            <DrawerDescription>
              Source Key <code>{row.key}</code> · {row.statusLabel} ·{" "}
              {row.validationStateLabel} · {row.originLabel}
            </DrawerDescription>
            {row.source.origin === "custom" &&
            row.source.document.selectedAccessPath.type === "profile_access_path" ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="absolute top-5 right-16"
                onClick={() => onEdit?.(row.source)}
              >
                <PencilIcon data-icon="inline-start" aria-hidden="true" />
                Bearbeiten
              </Button>
            ) : null}
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="absolute top-5 right-5"
              onClick={() => onOpenChange(false)}
            >
              <XIcon aria-hidden="true" />
              <span className="sr-only">Drawer schließen</span>
            </Button>
          </DrawerHeader>
          <div className="min-h-0 overflow-y-auto px-4 pb-4">
            <SourceDetails
              source={row.source}
              profilesByKey={profilesByKey}
              diagnostics={diagnostics}
              onUpdated={onUpdated}
            />
          </div>
        </DrawerContent>
      ) : null}
    </Drawer>
  );
}

type ProfileDetailsDrawerProps = {
  row: ProfileGridRow | null;
  diagnostics: StructuredDiagnostic[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function ProfileDetailsDrawer({
  row,
  diagnostics,
  open,
  onOpenChange,
}: ProfileDetailsDrawerProps) {
  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction="right">
      {row ? (
        <DrawerContent
          className="h-full data-[vaul-drawer-direction=right]:w-[min(calc(100vw-115px),960px)]
      data-[vaul-drawer-direction=right]:sm:max-w-none"
        >
          <DrawerHeader className="border-b pr-12">
            <DrawerTitle>{row.name}</DrawerTitle>
            <DrawerDescription>
              Profil-Key <code>{row.key}</code> · {row.kindLabel} ·{" "}
              deklarierter Support: {row.supportLabel} · {row.originLabel}
            </DrawerDescription>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="absolute top-5 right-5"
              onClick={() => onOpenChange(false)}
            >
              <XIcon aria-hidden="true" />
              <span className="sr-only">Drawer schließen</span>
            </Button>
          </DrawerHeader>
          <div className="min-h-0 overflow-y-auto px-4 pb-4">
            <ProfileDetails profile={row.profile} diagnostics={diagnostics} />
          </div>
        </DrawerContent>
      ) : null}
    </Drawer>
  );
}

type EvidenceBadgeSectionProps<TKind extends string> = {
  title: string;
  description: string;
  emptyLabel: string;
  evidence: Array<{
    kind: TKind;
    reference?: string;
    message?: string;
    summary?: string;
  }>;
  labelForKind: (kind: TKind) => string;
};

function EvidenceBadgeSection<TKind extends string>({
  title,
  description,
  emptyLabel,
  evidence,
  labelForKind,
}: EvidenceBadgeSectionProps<TKind>) {
  return (
    <section className="grid gap-2 rounded-lg border bg-muted/30 p-3">
      <div className="grid gap-1">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          {title}
        </h3>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      {evidence.length ? (
        <div className="flex flex-wrap gap-1.5">
          {evidence.map((item, index) => (
            <Badge
              key={`${item.kind}-${item.reference ?? item.message ?? index}`}
              variant="secondary"
              title={item.summary ?? item.message ?? item.reference}
            >
              {labelForKind(item.kind)}
            </Badge>
          ))}
        </div>
      ) : (
        <span className="text-xs text-muted-foreground">{emptyLabel}</span>
      )}
    </section>
  );
}

type ProfileVerificationSectionProps = {
  status: SourceProfileVerificationReportStatus | null;
  loading: boolean;
  running: boolean;
  error: string | null;
  canVerify: boolean;
  onVerify: () => void;
  onRefresh: () => void;
};

function ProfileVerificationSection({
  status,
  loading,
  running,
  error,
  canVerify,
  onVerify,
  onRefresh,
}: ProfileVerificationSectionProps) {
  const model = profileVerificationDisplayModel(status);
  const report = status?.report ?? null;
  const alertVariant =
    model.displayState === "verified"
      ? "success"
      : model.displayState === "failed"
        ? "destructive"
        : model.displayState === "stale"
          ? "warning"
          : "info";
  const StatusIcon = model.displayState === "verified" ? CheckCircle2Icon : AlertCircleIcon;

  return (
    <section className="grid gap-3 rounded-lg border bg-muted/30 p-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid gap-1">
          <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Profile Verification Check
          </h3>
          <p className="text-xs text-muted-foreground">
            Offline-/Fixture-basierter Verification Report. Dieser Abschnitt ist getrennt von Source Live Checks und mutiert den deklarierten Support nicht.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={loading || running}
            onClick={onRefresh}
          >
            <RefreshCwIcon data-icon="inline-start" aria-hidden="true" />
            Status laden
          </Button>
          {canVerify ? (
            <Button
              type="button"
              size="sm"
              disabled={loading || running}
              onClick={onVerify}
            >
              {running ? (
                <Loader2Icon
                  data-icon="inline-start"
                  className="animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <CheckCircle2Icon data-icon="inline-start" aria-hidden="true" />
              )}
              Prüfen
            </Button>
          ) : null}
        </div>
      </div>

      {loading ? (
        <Alert variant="info">
          <Loader2Icon className="size-4 animate-spin" aria-hidden="true" />
          <AlertTitle>Verification Report wird geladen</AlertTitle>
          <AlertDescription>Der neueste persistierte Report wird gelesen.</AlertDescription>
        </Alert>
      ) : null}

      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Verification Report konnte nicht geladen werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {!loading ? (
        <Alert variant={alertVariant}>
          <StatusIcon className="size-4" aria-hidden="true" />
          <AlertTitle>{model.displayLabel}</AlertTitle>
          <AlertDescription>
            {model.isFreshVerified
              ? "Nur dieser frische Report mit Effective Verification State verified wird als verifiziert dargestellt."
              : "Unknown, stale, failed oder not-applicable werden nicht als verifiziert dargestellt."}
          </AlertDescription>
        </Alert>
      ) : null}

      <dl className="grid gap-3 sm:grid-cols-2">
        <DetailRow label="Report-Zustand" value={model.reportStateLabel} />
        <DetailRow label="Report Result" value={model.reportResultLabel} />
        <DetailRow label="Effective Verification State" value={model.effectiveStateLabel} />
        <DetailRow
          label="Checked At"
          value={report?.checkedAt ? new Date(report.checkedAt).toLocaleString("de") : "—"}
        />
      </dl>

      {status?.freshness?.staleFingerprints.length ? (
        <div className="grid gap-2">
          <h4 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Stale-Fingerprints
          </h4>
          <div className="grid gap-2">
            {status.freshness.staleFingerprints.map((stale, index) => (
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

      <FixtureCheckResults
        fixtureChecks={model.fixtureChecks}
        fresh={model.reportState === "fresh"}
      />

      {model.diagnostics.length ? (
        <InlineDiagnostics
          title="Verification-Report-Diagnosen"
          diagnostics={model.diagnostics}
        />
      ) : null}
    </section>
  );
}

type FixtureCheckResultsProps = {
  fixtureChecks: FixtureCheckResult[];
  fresh: boolean;
};

function FixtureCheckResults({ fixtureChecks, fresh }: FixtureCheckResultsProps) {
  return (
    <div className="grid gap-2">
      <h4 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        Fixture Check Results
      </h4>
      {fixtureChecks.length ? (
        <div className="grid gap-2">
          {fixtureChecks.map((fixtureCheck) => (
            <div
              key={`${fixtureCheck.reference}-${fixtureCheck.accessPathKey ?? "none"}`}
              className="grid gap-2 rounded-md border bg-background p-2 text-xs"
            >
              <div className="flex flex-wrap gap-1">
                <Badge
                  variant={
                    fixtureCheck.result === "passed" && fresh
                      ? "success-light"
                      : fixtureCheck.result === "failed"
                        ? "destructive-light"
                        : "outline"
                  }
                >
                  {checkReportResultLabels[fixtureCheck.result]}
                </Badge>
                <Badge variant="outline">{fixtureCheck.reference}</Badge>
                {fixtureCheck.accessPathKey ? (
                  <Badge variant="outline">Access Path {fixtureCheck.accessPathKey}</Badge>
                ) : null}
              </div>
              <div className="flex flex-wrap gap-1">
                <CoverageBadge
                  label="Posting Discovery"
                  covered={fixtureCheck.coverage?.postingDiscovery}
                  fresh={fresh}
                />
                <CoverageBadge
                  label="Detail Description Text"
                  covered={fixtureCheck.coverage?.postingDetailDescriptionText}
                  fresh={fresh}
                />
              </div>
            </div>
          ))}
        </div>
      ) : (
        <p className="text-xs text-muted-foreground">
          Keine Fixture Check Results im Report. Eine deklarierte Fixture-Evidenz alleine wird hier nicht als bestanden gerendert.
        </p>
      )}
    </div>
  );
}

type CoverageBadgeProps = {
  label: string;
  covered?: boolean;
  fresh: boolean;
};

function CoverageBadge({ label, covered, fresh }: CoverageBadgeProps) {
  return (
    <Badge variant={covered && fresh ? "success-light" : "outline"}>
      {label}: {covered ? "abgedeckt" : "nicht abgedeckt"}
    </Badge>
  );
}

type SourceLiveCheckSectionProps = {
  source: RegistrySource;
  onUpdated?: () => Promise<unknown> | unknown;
};

function SourceLiveCheckSection({
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

type SourceDetailsProps = {
  source: RegistrySource;
  profilesByKey: Map<string, RegistrySourceProfile>;
  diagnostics: StructuredDiagnostic[];
  onUpdated?: () => Promise<unknown> | unknown;
};

function SourceDetails({
  source,
  profilesByKey,
  diagnostics,
  onUpdated,
}: SourceDetailsProps) {
  const selectedAccessPath = source.document.selectedAccessPath;
  const resolution = resolveSource(source, profilesByKey);
  const validationDiagnostics = source.validationState.diagnostics ?? [];

  return (
    <div className="grid gap-4 py-4 text-sm">
      {diagnostics.length ? (
        <InlineDiagnostics
          title="Diagnosen zu dieser Source"
          diagnostics={diagnostics}
        />
      ) : null}
      {validationDiagnostics.length ? (
        <InlineDiagnostics
          title="Validation-State-Diagnosen"
          diagnostics={validationDiagnostics}
        />
      ) : null}

      <SourceLiveCheckSection source={source} onUpdated={onUpdated} />

      <dl className="grid gap-3 sm:grid-cols-2">
        <DetailRow label="Source Key" value={source.document.key} mono />
        <DetailRow label="Name" value={source.document.name} />
        <DetailRow
          label="Source Status"
          value={sourceStatusLabels[source.document.status]}
        />
        <DetailRow
          label="Validation State"
          value={validationStateLabels[source.validationState.state]}
        />
        <DetailRow
          label="Kann kompilieren"
          value={source.validationState.canCompile ? "Ja" : "Nein"}
        />
        <DetailRow
          label="Kann ausführen"
          value={source.validationState.canExecute ? "Ja" : "Nein"}
        />
        <DetailRow
          label="Deklarierter Profil-/Access-Path-Support"
          value={
            resolution.supportLevel
              ? supportLevelLabels[resolution.supportLevel]
              : "—"
          }
        />
        <DetailRow label="Ursprung" value={originLabels[source.origin]} />
        <DetailRow label="Registry-Dokument" value={source.path} mono />
      </dl>

      <SchemaValuePreview
        title="sourceConfig"
        description="Stabile Zugriffskonfiguration der Source. Search Request Kriterien gehören nicht hierher."
        value={source.document.sourceConfig}
        schema={resolution.effectiveSourceConfigSchema}
      />
      <SchemaValuePreview
        title="Effektives sourceConfigSchema"
        description="Profil- und Access-Path-Schema, wie die Registry sie für diese Source zusammenführt."
        value={resolution.effectiveSourceConfigSchema}
      />

      <AccessPathDetails
        selectedAccessPath={selectedAccessPath}
        resolution={resolution}
      />

      <OptionalSchemaValuePreview
        title="sourceOverrides"
        description="Kontrollierte Source-spezifische Verhaltensänderungen für den ausgewählten Profilpfad."
        value={source.document.sourceOverrides}
        schemaRef={profileDslSchemaRefs.sourceOverrides}
      />
      <OptionalSchemaValuePreview
        title="sourceSupport"
        description="Support-Metadaten für Source-owned Access Paths."
        value={source.document.sourceSupport}
        schemaRef={profileDslSchemaRefs.supportMetadata}
      />
      <OptionalJsonPreview
        title="Source-Diagnosen im Dokument"
        description="Im Source-Dokument gespeicherte strukturierte Diagnosen."
        value={source.document.diagnostics}
      />
    </div>
  );
}

type ProfileDetailsProps = {
  profile: RegistrySourceProfile;
  diagnostics: StructuredDiagnostic[];
};

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function ProfileDetails({ profile, diagnostics }: ProfileDetailsProps) {
  const accessPaths = [...profile.document.accessPaths].sort((left, right) =>
    left.key.localeCompare(right.key, "de"),
  );
  const [verificationStatus, setVerificationStatus] =
    useState<SourceProfileVerificationReportStatus | null>(null);
  const [verificationLoading, setVerificationLoading] = useState(true);
  const [verificationRunning, setVerificationRunning] = useState(false);
  const [verificationError, setVerificationError] = useState<string | null>(null);
  const profileKey = profile.document.key;

  const refreshVerificationStatus = useCallback(async () => {
    setVerificationLoading(true);
    setVerificationError(null);
    try {
      setVerificationStatus(await getSourceProfileVerificationReportStatus(profileKey));
    } catch (unknownError) {
      setVerificationStatus(null);
      setVerificationError(errorMessage(unknownError));
    } finally {
      setVerificationLoading(false);
    }
  }, [profileKey]);

  useEffect(() => {
    void refreshVerificationStatus();
  }, [refreshVerificationStatus]);

  const handleVerify = useCallback(async () => {
    setVerificationRunning(true);
    setVerificationError(null);
    try {
      await verifySourceProfile(profileKey);
      const nextStatus = await getSourceProfileVerificationReportStatus(profileKey);
      setVerificationStatus(nextStatus);
      toast.success("Source Profile geprüft", {
        description: `Verification Report für ${profileKey} wurde aktualisiert.`,
      });
    } catch (unknownError) {
      const message = errorMessage(unknownError);
      setVerificationError(message);
      toast.error("Source Profile konnte nicht geprüft werden", {
        description: message,
      });
    } finally {
      setVerificationRunning(false);
    }
  }, [profileKey]);

  return (
    <div className="grid gap-4 py-4 text-sm">
      {diagnostics.length ? (
        <InlineDiagnostics
          title="Diagnosen zu diesem Source Profile"
          diagnostics={diagnostics}
        />
      ) : null}
      {profile.document.diagnostics?.length ? (
        <InlineDiagnostics
          title="Im Profil gespeicherte Diagnosen"
          diagnostics={profile.document.diagnostics}
        />
      ) : null}

      <ProfileVerificationSection
        status={verificationStatus}
        loading={verificationLoading}
        running={verificationRunning}
        error={verificationError}
        canVerify={profile.origin === "custom"}
        onVerify={handleVerify}
        onRefresh={refreshVerificationStatus}
      />

      <dl className="grid gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-2">
        <DetailRow label="Profil-Key" value={profile.document.key} mono />
        <DetailRow label="Name" value={profile.document.name} />
        <DetailRow
          label="Kind"
          value={profileKindLabels[profile.document.kind]}
        />
        <DetailRow
          label="Deklarierter Support"
          value={supportLevelLabels[profile.document.support.level]}
        />
        <DetailRow label="Ursprung" value={originLabels[profile.origin]} />
        <DetailRow label="Registry-Dokument" value={profile.path} mono />
      </dl>

      {profile.document.description ? (
        <p className="text-muted-foreground">{profile.document.description}</p>
      ) : null}
      <div className="flex flex-wrap gap-1">
        {profile.document.support.knownIssues?.map((issue, index) => (
          <Badge key={`${issue.message}-${index}`} variant="warning-light">
            {issue.scope ? `${issue.scope}: ` : ""}
            {issue.message}
          </Badge>
        ))}
      </div>

      <EvidenceBadgeSection
        title="Support-Evidenz"
        description="Deklarierte Support-Evidenz. Fixture bedeutet hier nur: Fixture Evidence ist angegeben, nicht dass sie bestanden hat."
        emptyLabel="Keine Support-Evidenz deklariert."
        evidence={profile.document.support.evidence ?? []}
        labelForKind={(kind) => supportEvidenceKindLabels[kind]}
      />
      <EvidenceBadgeSection
        title="Detection-Evidenz"
        description="Detection-Evidenz gehört zu detect.evidence und ist getrennt von Support-Evidenz. URL bleibt hier gültige Detection-Evidenz."
        emptyLabel="Keine Detection-Evidenz deklariert."
        evidence={profile.document.detect?.evidence ?? []}
        labelForKind={(kind) => detectionEvidenceKindLabels[kind]}
      />

      <OptionalSchemaValuePreview
        title="support"
        description="Support Level, bekannte Einschränkungen und Evidenz des Source Profile."
        value={profile.document.support}
        schemaRef={profileDslSchemaRefs.supportMetadata}
      />
      <OptionalSchemaValuePreview
        title="Profil sourceConfigSchema"
        description="Schema-Anteil, der für alle Access Paths dieses Profils gilt."
        value={profile.document.sourceConfigSchema}
      />
      <OptionalSchemaValuePreview
        title="Detection-Regeln"
        description="Regeln, wie dieses Profil bei eingereichten URLs eine Source Proposal erzeugt."
        value={profile.document.detect}
        schemaRef={profileDslSchemaRefs.detection}
      />

      <div className="grid gap-2">
        <h3 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          Access Paths
        </h3>
        {accessPaths.map((accessPath) => (
          <ProfileAccessPathRow key={accessPath.key} accessPath={accessPath} />
        ))}
      </div>
    </div>
  );
}
