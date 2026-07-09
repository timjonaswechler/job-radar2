import {
  checkReportResultLabels,
  sourceLiveCheckDisplayStateLabels,
  sourceLiveCheckReportStateLabels,
  type SourceLiveCheckDisplayState,
} from "@/features/sources/labels";
import type {
  CheckReport,
  SourceLiveCheckReportStatus,
  SourceStatus,
  StructuredDiagnostic,
} from "@/lib/api/sources";

export type SourceLiveCheckDisplayModel = {
  displayState: SourceLiveCheckDisplayState;
  displayLabel: string;
  reportState: "fresh" | "stale" | "unknown";
  reportStateLabel: string;
  reportResultLabel: string;
  diagnostics: StructuredDiagnostic[];
  staleFingerprints: NonNullable<SourceLiveCheckReportStatus["freshness"]>["staleFingerprints"];
};

export type SourceLiveCheckActionKind =
  | "check"
  | "check_and_activate"
  | "check_and_reactivate";

export type SourceLiveCheckAction = {
  kind: SourceLiveCheckActionKind;
  label: string;
  description: string;
};

export function sourceLiveCheckDisplayModel(
  status: SourceLiveCheckReportStatus | null | undefined,
): SourceLiveCheckDisplayModel {
  const reportState = status?.state ?? "unknown";
  const report = status?.report ?? null;
  const displayState = sourceLiveCheckDisplayState(reportState, report);

  return {
    displayState,
    displayLabel: sourceLiveCheckDisplayStateLabels[displayState],
    reportState,
    reportStateLabel: sourceLiveCheckReportStateLabels[reportState],
    reportResultLabel: report ? checkReportResultLabels[report.result] : "Kein Report",
    diagnostics: report?.diagnostics ?? [],
    staleFingerprints: status?.freshness?.staleFingerprints ?? [],
  };
}

export function sourceLiveCheckActionsForSource(
  status: SourceStatus,
): SourceLiveCheckAction[] {
  const actions: SourceLiveCheckAction[] = [
    {
      kind: "check",
      label: "Prüfen",
      description: "Führt eine status-neutrale Source Live Check aus.",
    },
  ];

  if (status === "draft") {
    actions.push({
      kind: "check_and_activate",
      label: "Prüfen & Aktivieren",
      description: "Aktiviert den Entwurf nur nach bestandener Live-Prüfung.",
    });
  }

  if (status === "disabled") {
    actions.push({
      kind: "check_and_reactivate",
      label: "Prüfen & Reaktivieren",
      description: "Reaktiviert die deaktivierte Source nur nach bestandener Live-Prüfung.",
    });
  }

  return actions;
}

function sourceLiveCheckDisplayState(
  reportState: "fresh" | "stale" | "unknown",
  report: CheckReport | null,
): SourceLiveCheckDisplayState {
  if (reportState === "stale") return "stale";
  if (reportState === "unknown" || !report) return "unknown";
  return report.result === "passed" ? "passed" : "failed";
}
