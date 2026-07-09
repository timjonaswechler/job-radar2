import {
  originLabels,
  supportLevelLabels,
  validationStateLabels,
} from "@/features/sources/labels";
import { sourceStatusLabels } from "@/features/sources/status";
import { resolveSource } from "@/features/sources/view-model/registry-resolution";
import {
  isSourceDependencyDiagnostic,
  uniqueDiagnostics,
} from "@/features/sources/view-model/diagnostics";
import type {
  JsonValue,
  RegistrySource,
  RegistrySourceProfile,
  SelectedAccessPath,
  SourceRegistryDocumentOrigin,
  SourceStatus,
  StructuredDiagnostic,
  SupportLevel,
  ValidationStateKind,
} from "@/lib/api/sources";

export type SourceRegistryRowHealth = "valid" | "dependency_warning" | "invalid";

export type SourceRegistryRowDiagnosticSummary = {
  health: SourceRegistryRowHealth;
  diagnosticsCount: number;
  ownDiagnosticsCount: number;
  dependencyDiagnosticsCount: number;
};

export type SourceGridRow = {
  key: string;
  name: string;
  status: SourceStatus;
  statusLabel: string;
  validationState: ValidationStateKind;
  validationStateLabel: string;
  supportLevel: SupportLevel | null;
  supportLabel: string;
  origin: SourceRegistryDocumentOrigin;
  originLabel: string;
  accessPathLabel: string;
  profileLabel: string;
  capabilitiesSummary: string;
  configSummary: string;
  health: SourceRegistryRowHealth;
  diagnosticsCount: number;
  ownDiagnosticsCount: number;
  dependencyDiagnosticsCount: number;
  path: string;
  searchText: string;
  source: RegistrySource;
};

export type SourceGridFilters = {
  searchQuery: string;
  statuses: SourceStatus[];
  origins: SourceRegistryDocumentOrigin[];
  diagnosticsOnly: boolean;
};

export function createSourceGridRows(
  sources: RegistrySource[],
  profilesByKey: Map<string, RegistrySourceProfile>,
  diagnosticsBySourceKey: Map<string, StructuredDiagnostic[]>,
): SourceGridRow[] {
  return sources.map((source) => {
    const resolution = resolveSource(source, profilesByKey);
    const selectedAccessPath = source.document.selectedAccessPath;
    const diagnostics = uniqueDiagnostics([
      ...(diagnosticsBySourceKey.get(source.document.key) ?? []),
      ...(source.validationState.diagnostics ?? []),
      ...(source.document.diagnostics ?? []),
    ]);
    const diagnosticSummary = classifySourceRegistryRowHealth(source, diagnostics);
    const accessPathLabel = accessPathSummary(selectedAccessPath);
    const profileLabel =
      selectedAccessPath.type === "profile_access_path"
        ? `${selectedAccessPath.profileKey} / ${selectedAccessPath.pathKey}`
        : "Source-owned";
    const configSummary = jsonObjectSummary(source.document.sourceConfig);
    const statusLabel = sourceStatusLabels[source.document.status];
    const validationStateLabel = validationStateLabels[source.validationState.state];
    const originLabel = originLabels[source.origin];
    const supportLabel = resolution.supportLevel
      ? supportLevelLabels[resolution.supportLevel]
      : "—";
    const capabilitiesSummary = summarizeList(resolution.capabilities, "keine Fähigkeiten");
    const searchText = [
      source.document.key,
      source.document.name,
      statusLabel,
      source.document.status,
      validationStateLabel,
      source.validationState.state,
      originLabel,
      source.origin,
      accessPathLabel,
      profileLabel,
      resolution.profile?.document.name ?? "",
      supportLabel,
      capabilitiesSummary,
      configSummary,
      source.path,
    ]
      .join(" ")
      .toLocaleLowerCase("de");

    return {
      key: source.document.key,
      name: source.document.name,
      status: source.document.status,
      statusLabel,
      validationState: source.validationState.state,
      validationStateLabel,
      supportLevel: resolution.supportLevel,
      supportLabel,
      origin: source.origin,
      originLabel,
      accessPathLabel,
      profileLabel,
      capabilitiesSummary,
      configSummary,
      health: diagnosticSummary.health,
      diagnosticsCount: diagnosticSummary.diagnosticsCount,
      ownDiagnosticsCount: diagnosticSummary.ownDiagnosticsCount,
      dependencyDiagnosticsCount: diagnosticSummary.dependencyDiagnosticsCount,
      path: source.path,
      searchText,
      source,
    };
  });
}

export function classifySourceRegistryRowHealth(
  source: RegistrySource,
  diagnostics: StructuredDiagnostic[],
): SourceRegistryRowDiagnosticSummary {
  const dependencyDiagnosticsCount = diagnostics.filter(isSourceDependencyDiagnostic).length;
  const ownDiagnosticsCount = diagnostics.length - dependencyDiagnosticsCount;
  let health: SourceRegistryRowHealth = "valid";

  if (source.validationState.state === "invalid" || ownDiagnosticsCount > 0) {
    health = "invalid";
  } else if (dependencyDiagnosticsCount > 0 || source.validationState.state === "unknown") {
    health = "dependency_warning";
  }

  return {
    health,
    diagnosticsCount: diagnostics.length,
    ownDiagnosticsCount,
    dependencyDiagnosticsCount,
  };
}

export function filterSourceGridRows(
  rows: SourceGridRow[],
  filters: SourceGridFilters,
): SourceGridRow[] {
  const normalizedSearch = filters.searchQuery.trim().toLocaleLowerCase("de");

  return rows.filter(
    (row) =>
      (!normalizedSearch || row.searchText.includes(normalizedSearch)) &&
      (!filters.statuses.length || filters.statuses.includes(row.status)) &&
      (!filters.origins.length || filters.origins.includes(row.origin)) &&
      (!filters.diagnosticsOnly || row.diagnosticsCount > 0),
  );
}

export function countSourceStatuses(rows: SourceGridRow[]) {
  const counts: Record<SourceStatus, number> = {
    draft: 0,
    active: 0,
    disabled: 0,
  };
  rows.forEach((row) => {
    counts[row.status] += 1;
  });
  return counts;
}

export function countOrigins(
  rows: Array<{ origin: SourceRegistryDocumentOrigin }>,
): Record<SourceRegistryDocumentOrigin, number> {
  const counts: Record<SourceRegistryDocumentOrigin, number> = {
    built_in: 0,
    custom: 0,
  };
  rows.forEach((row) => {
    counts[row.origin] += 1;
  });
  return counts;
}

export function sourceStatusEntries() {
  return Object.entries(sourceStatusLabels) as Array<[SourceStatus, string]>;
}

export function sourceOriginEntries() {
  return Object.entries(originLabels) as Array<
    [SourceRegistryDocumentOrigin, string]
  >;
}

function accessPathSummary(selectedAccessPath: SelectedAccessPath) {
  if (selectedAccessPath.type === "source_owned_access_path") {
    return `Source-owned · ${selectedAccessPath.key}`;
  }

  return `Profil ${selectedAccessPath.profileKey} · Pfad ${selectedAccessPath.pathKey}`;
}

function jsonObjectSummary(value: JsonValue) {
  const keys = jsonObjectKeys(value);
  if (!keys.length) return "{}";
  return summarizeList(keys, "{}");
}

function jsonObjectKeys(value: JsonValue | undefined) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  return Object.keys(value);
}

function summarizeList(values: string[], emptyLabel: string) {
  if (!values.length) return emptyLabel;
  if (values.length <= 3) return values.join(", ");
  return `${values.slice(0, 3).join(", ")} +${values.length - 3}`;
}
