import { originLabels, profileKindLabels, supportLevelLabels, validationStateLabels } from "@/features/sources/labels";
import { effectiveSourceConfigSchema } from "@/features/sources/source-config-schema";
import { sourceStatusLabels } from "@/features/sources/status";
import type {
  JsonValue,
  ProfileAccessPathDefinition,
  RegistrySource,
  RegistrySourceProfile,
  SelectedAccessPath,
  SourceOwnedSelectedAccessPath,
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
  SourceStatus,
  StructuredDiagnostic,
  SupportLevel,
  ValidationStateKind,
} from "@/lib/api/sources";

import {
  isSourceDependencyDiagnostic,
  uniqueDiagnostics,
} from "@/features/sources/registry-view-model/diagnostics";
export {
  buildDiagnosticIndex,
  diagnosticDocumentKey,
  diagnosticDocumentKind,
  diagnosticDocumentOrigin,
  diagnosticDocumentPath,
} from "@/features/sources/registry-view-model/diagnostics";
export type { DiagnosticIndex } from "@/features/sources/registry-view-model/diagnostics";

export type SourceRegistryInventory = {
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  diagnostics: StructuredDiagnostic[];
};

export type RegistryRowHealth = "valid" | "dependency_warning" | "invalid";

export type RegistryRowDiagnosticSummary = {
  health: RegistryRowHealth;
  diagnosticsCount: number;
  ownDiagnosticsCount: number;
  dependencyDiagnosticsCount: number;
};

export type SourceResolution = {
  profile: RegistrySourceProfile | null;
  profileAccessPath: ProfileAccessPathDefinition | null;
  sourceOwnedAccessPath: SourceOwnedSelectedAccessPath | null;
  effectiveSourceConfigSchema: JsonValue;
  supportLevel: SupportLevel | null;
  capabilities: string[];
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
  health: RegistryRowHealth;
  diagnosticsCount: number;
  ownDiagnosticsCount: number;
  dependencyDiagnosticsCount: number;
  path: string;
  searchText: string;
  source: RegistrySource;
};

export type ProfileGridRow = {
  key: string;
  name: string;
  kind: SourceProfileKind;
  kindLabel: string;
  supportLevel: SupportLevel;
  supportLabel: string;
  origin: SourceRegistryDocumentOrigin;
  originLabel: string;
  accessPathCount: number;
  capabilitiesSummary: string;
  schemaSummary: string;
  health: RegistryRowHealth;
  diagnosticsCount: number;
  ownDiagnosticsCount: number;
  dependencyDiagnosticsCount: number;
  path: string;
  searchText: string;
  profile: RegistrySourceProfile;
};

export type SourceGridFilters = {
  searchQuery: string;
  statuses: SourceStatus[];
  origins: SourceRegistryDocumentOrigin[];
  diagnosticsOnly: boolean;
};

export type ProfileGridFilters = {
  searchQuery: string;
  kinds: SourceProfileKind[];
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

export function createProfileGridRows(
  profiles: RegistrySourceProfile[],
  diagnosticsByProfileKey: Map<string, StructuredDiagnostic[]>,
): ProfileGridRow[] {
  return profiles.map((profile) => {
    const diagnostics = uniqueDiagnostics([
      ...(diagnosticsByProfileKey.get(profile.document.key) ?? []),
      ...(profile.document.diagnostics ?? []),
      ...profile.document.accessPaths.flatMap((accessPath) => accessPath.diagnostics ?? []),
    ]);
    const diagnosticSummary = classifyProfileRegistryRowHealth(diagnostics);
    const kindLabel = profileKindLabels[profile.document.kind];
    const supportLabel = supportLevelLabels[profile.document.support.level];
    const originLabel = originLabels[profile.origin];
    const schemaSummary = profileSchemaSummary(profile);
    const capabilitiesSummary = summarizeList(profileCapabilities(profile), "keine Fähigkeiten");
    const searchText = [
      profile.document.key,
      profile.document.name,
      kindLabel,
      profile.document.kind,
      supportLabel,
      profile.document.support.level,
      originLabel,
      profile.origin,
      capabilitiesSummary,
      schemaSummary,
      profile.path,
      profile.document.accessPaths.map((accessPath) => accessPath.key).join(" "),
    ]
      .join(" ")
      .toLocaleLowerCase("de");

    return {
      key: profile.document.key,
      name: profile.document.name,
      kind: profile.document.kind,
      kindLabel,
      supportLevel: profile.document.support.level,
      supportLabel,
      origin: profile.origin,
      originLabel,
      accessPathCount: profile.document.accessPaths.length,
      capabilitiesSummary,
      schemaSummary,
      health: diagnosticSummary.health,
      diagnosticsCount: diagnosticSummary.diagnosticsCount,
      ownDiagnosticsCount: diagnosticSummary.ownDiagnosticsCount,
      dependencyDiagnosticsCount: diagnosticSummary.dependencyDiagnosticsCount,
      path: profile.path,
      searchText,
      profile,
    };
  });
}

export function classifySourceRegistryRowHealth(
  source: RegistrySource,
  diagnostics: StructuredDiagnostic[],
): RegistryRowDiagnosticSummary {
  const dependencyDiagnosticsCount = diagnostics.filter(isSourceDependencyDiagnostic).length;
  const ownDiagnosticsCount = diagnostics.length - dependencyDiagnosticsCount;
  let health: RegistryRowHealth = "valid";

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

export function classifyProfileRegistryRowHealth(
  diagnostics: StructuredDiagnostic[],
): RegistryRowDiagnosticSummary {
  return {
    health: diagnostics.some((diagnostic) => diagnostic.severity === "error")
      ? "invalid"
      : diagnostics.length > 0
        ? "dependency_warning"
        : "valid",
    diagnosticsCount: diagnostics.length,
    ownDiagnosticsCount: diagnostics.length,
    dependencyDiagnosticsCount: 0,
  };
}

export function filterSourceGridRows(
  rows: SourceGridRow[],
  filters: SourceGridFilters,
): SourceGridRow[] {
  const normalizedSearch = normalizeRegistrySearchQuery(filters.searchQuery);

  return rows.filter(
    (row) =>
      matchesRegistrySearch(row.searchText, normalizedSearch) &&
      matchesSelectedValue(filters.statuses, row.status) &&
      matchesSelectedValue(filters.origins, row.origin) &&
      matchesDiagnosticsFilter(row.diagnosticsCount, filters.diagnosticsOnly),
  );
}

export function filterProfileGridRows(
  rows: ProfileGridRow[],
  filters: ProfileGridFilters,
): ProfileGridRow[] {
  const normalizedSearch = normalizeRegistrySearchQuery(filters.searchQuery);

  return rows.filter(
    (row) =>
      matchesRegistrySearch(row.searchText, normalizedSearch) &&
      matchesSelectedValue(filters.kinds, row.kind) &&
      matchesSelectedValue(filters.origins, row.origin) &&
      matchesDiagnosticsFilter(row.diagnosticsCount, filters.diagnosticsOnly),
  );
}

export function resolveSource(
  source: RegistrySource,
  profilesByKey: Map<string, RegistrySourceProfile>,
): SourceResolution {
  const selectedAccessPath = source.document.selectedAccessPath;

  if (selectedAccessPath.type === "source_owned_access_path") {
    return {
      profile: null,
      profileAccessPath: null,
      sourceOwnedAccessPath: selectedAccessPath,
      effectiveSourceConfigSchema: effectiveSourceConfigSchema(
        undefined,
        selectedAccessPath.sourceConfigSchema,
      ),
      supportLevel: source.document.sourceSupport?.level ?? null,
      capabilities: accessPathCapabilities(selectedAccessPath),
    };
  }

  const profile = profilesByKey.get(selectedAccessPath.profileKey) ?? null;
  const profileAccessPath =
    profile?.document.accessPaths.find(
      (accessPath) => accessPath.key === selectedAccessPath.pathKey,
    ) ?? null;

  return {
    profile,
    profileAccessPath,
    sourceOwnedAccessPath: null,
    effectiveSourceConfigSchema: effectiveSourceConfigSchema(
      profile?.document.sourceConfigSchema,
      profileAccessPath?.sourceConfigSchema,
    ),
    supportLevel: profile?.document.support.level ?? null,
    capabilities: profileAccessPath ? accessPathCapabilities(profileAccessPath) : [],
  };
}

export function countSourceStatuses(rows: SourceGridRow[]) {
  const counts = zeroSourceStatusCounts();
  rows.forEach((row) => {
    counts[row.status] += 1;
  });
  return counts;
}

export function countProfileKinds(rows: ProfileGridRow[]) {
  const counts = zeroProfileKindCounts();
  rows.forEach((row) => {
    counts[row.kind] += 1;
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

export function profileKindEntries() {
  return Object.entries(profileKindLabels) as Array<[SourceProfileKind, string]>;
}

export function originEntries() {
  return Object.entries(originLabels) as Array<
    [SourceRegistryDocumentOrigin, string]
  >;
}

export function diagnosticCountLabel(count: number) {
  return `${count} Diagnose${count === 1 ? "" : "n"}`;
}

export function formatBoolean(value: boolean) {
  return value ? "Ja" : "Nein";
}

function normalizeRegistrySearchQuery(searchQuery: string) {
  return searchQuery.trim().toLocaleLowerCase("de");
}

function matchesRegistrySearch(searchText: string, normalizedSearch: string) {
  return !normalizedSearch || searchText.includes(normalizedSearch);
}

function matchesSelectedValue<T>(selectedValues: T[], value: T) {
  return !selectedValues.length || selectedValues.includes(value);
}

function matchesDiagnosticsFilter(
  diagnosticsCount: number,
  diagnosticsOnly: boolean,
) {
  return !diagnosticsOnly || diagnosticsCount > 0;
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

function profileSchemaSummary(profile: RegistrySourceProfile) {
  const parts = [
    profile.document.sourceConfigSchema ? "Profil-Schema" : null,
    profile.document.detect ? "Detection" : null,
  ].filter(Boolean);

  const pathSchemaCount = profile.document.accessPaths.filter(
    (accessPath) => accessPath.sourceConfigSchema,
  ).length;

  if (pathSchemaCount) {
    parts.push(
      `${pathSchemaCount} Pfad-Schema${pathSchemaCount === 1 ? "" : "s"}`,
    );
  }

  return parts.join(" · ") || "keine Zusatzblöcke";
}

function profileCapabilities(profile: RegistrySourceProfile) {
  return unique(profile.document.accessPaths.flatMap(accessPathCapabilities));
}

function accessPathCapabilities(
  accessPath: ProfileAccessPathDefinition | SourceOwnedSelectedAccessPath,
) {
  return [
    accessPath.postingDiscovery ? "postingDiscovery" : null,
    accessPath.postingDetail ? "postingDetail" : null,
  ].filter(Boolean) as string[];
}

function summarizeList(values: string[], emptyLabel: string) {
  if (!values.length) return emptyLabel;
  if (values.length <= 3) return values.join(", ");
  return `${values.slice(0, 3).join(", ")} +${values.length - 3}`;
}

function unique(values: string[]) {
  return [...new Set(values)];
}

function zeroSourceStatusCounts(): Record<SourceStatus, number> {
  return {
    draft: 0,
    active: 0,
    disabled: 0,
  };
}

function zeroProfileKindCounts(): Record<SourceProfileKind, number> {
  return {
    recruiting_system: 0,
    job_portal: 0,
    website_family: 0,
    career_site: 0,
    generic: 0,
  };
}
