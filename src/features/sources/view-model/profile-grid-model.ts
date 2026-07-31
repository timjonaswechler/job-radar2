import {
  detectionEvidenceKindLabels,
  originLabels,
  profileKindLabels,
  supportEvidenceKindLabels,
  supportLevelLabels,
} from "@/features/sources/labels";
import { uniqueDiagnostics } from "@/features/sources/view-model/diagnostics";
import type {
  DetectionEvidenceKind,
  ProfileAccessPathDefinition,
  InstalledProfileWithDefinition,
  SourceProfileKind,
  SourceRegistryDocumentOrigin,
  StructuredDiagnostic,
  SupportEvidenceKind,
  SupportLevel,
} from "@/lib/api/sources";

export type ProfileRegistryRowHealth =
  | "valid"
  | "dependency_warning"
  | "invalid";

export type ProfileGridRow = {
  key: string;
  name: string;
  kind: SourceProfileKind;
  kindLabel: string;
  supportLevel: SupportLevel;
  supportLabel: string;
  supportEvidenceKinds: SupportEvidenceKind[];
  supportEvidenceLabels: string[];
  supportEvidenceSummary: string;
  detectionEvidenceKinds: DetectionEvidenceKind[];
  detectionEvidenceLabels: string[];
  detectionEvidenceSummary: string;
  origin: SourceRegistryDocumentOrigin;
  originLabel: string;
  accessPathCount: number;
  capabilitiesSummary: string;
  schemaSummary: string;
  health: ProfileRegistryRowHealth;
  diagnosticsCount: number;
  ownDiagnosticsCount: number;
  dependencyDiagnosticsCount: number;
  fileName: string;
  searchText: string;
  profile: InstalledProfileWithDefinition;
};

export type ProfileGridFilters = {
  searchQuery: string;
  kinds: SourceProfileKind[];
  origins: SourceRegistryDocumentOrigin[];
  diagnosticsOnly: boolean;
};

export function createProfileGridRows(
  profiles: InstalledProfileWithDefinition[],
  diagnosticsByProfileKey: Map<string, StructuredDiagnostic[]>,
): ProfileGridRow[] {
  return profiles.map((profile) => {
    const diagnostics = uniqueDiagnostics([
      ...(diagnosticsByProfileKey.get(profile.definition.key) ?? []),
      ...profile.definition.accessPaths.flatMap(
        (accessPath) => accessPath.diagnostics ?? [],
      ),
    ]);
    const diagnosticSummary = classifyProfileRegistryRowHealth(diagnostics);
    const kindLabel = profileKindLabels[profile.definition.kind];
    const supportLabel = supportLevelLabels[profile.definition.support.level];
    const supportEvidenceKinds = profileSupportEvidenceKinds(profile);
    const supportEvidenceLabels = supportEvidenceKinds.map(
      (kind) => supportEvidenceKindLabels[kind],
    );
    const supportEvidenceSummary = summarizeList(
      supportEvidenceLabels,
      "keine Support-Evidenz",
    );
    const detectionEvidenceKinds = profileDetectionEvidenceKinds(profile);
    const detectionEvidenceLabels = detectionEvidenceKinds.map(
      (kind) => detectionEvidenceKindLabels[kind],
    );
    const detectionEvidenceSummary = summarizeList(
      detectionEvidenceLabels,
      "keine Detection-Evidenz",
    );
    const originLabel = originLabels[profile.origin];
    const schemaSummary = profileSchemaSummary(profile);
    const capabilitiesSummary = summarizeList(
      profileCapabilities(profile),
      "keine Fähigkeiten",
    );
    const searchText = [
      profile.definition.key,
      profile.definition.name,
      kindLabel,
      profile.definition.kind,
      supportLabel,
      profile.definition.support.level,
      supportEvidenceSummary,
      supportEvidenceKinds.join(" "),
      detectionEvidenceSummary,
      detectionEvidenceKinds.join(" "),
      originLabel,
      profile.origin,
      capabilitiesSummary,
      schemaSummary,
      profile.fileName,
      profile.definition.accessPaths.map((accessPath) => accessPath.key).join(" "),
    ]
      .join(" ")
      .toLocaleLowerCase("de");

    return {
      key: profile.definition.key,
      name: profile.definition.name,
      kind: profile.definition.kind,
      kindLabel,
      supportLevel: profile.definition.support.level,
      supportLabel,
      supportEvidenceKinds,
      supportEvidenceLabels,
      supportEvidenceSummary,
      detectionEvidenceKinds,
      detectionEvidenceLabels,
      detectionEvidenceSummary,
      origin: profile.origin,
      originLabel,
      accessPathCount: profile.definition.accessPaths.length,
      capabilitiesSummary,
      schemaSummary,
      health: diagnosticSummary.health,
      diagnosticsCount: diagnosticSummary.diagnosticsCount,
      ownDiagnosticsCount: diagnosticSummary.ownDiagnosticsCount,
      dependencyDiagnosticsCount: diagnosticSummary.dependencyDiagnosticsCount,
      fileName: profile.fileName,
      searchText,
      profile,
    };
  });
}

export function classifyProfileRegistryRowHealth(
  diagnostics: StructuredDiagnostic[],
) {
  return {
    health: (diagnostics.some((diagnostic) => diagnostic.severity === "error")
      ? "invalid"
      : diagnostics.length > 0
        ? "dependency_warning"
        : "valid") as ProfileRegistryRowHealth,
    diagnosticsCount: diagnostics.length,
    ownDiagnosticsCount: diagnostics.length,
    dependencyDiagnosticsCount: 0,
  };
}

export function filterProfileGridRows(
  rows: ProfileGridRow[],
  filters: ProfileGridFilters,
): ProfileGridRow[] {
  const normalizedSearch = filters.searchQuery.trim().toLocaleLowerCase("de");

  return rows.filter(
    (row) =>
      (!normalizedSearch || row.searchText.includes(normalizedSearch)) &&
      (!filters.kinds.length || filters.kinds.includes(row.kind)) &&
      (!filters.origins.length || filters.origins.includes(row.origin)) &&
      (!filters.diagnosticsOnly || row.diagnosticsCount > 0),
  );
}

export function countProfileKinds(rows: ProfileGridRow[]) {
  const counts: Record<SourceProfileKind, number> = {
    recruiting_system: 0,
    job_portal: 0,
    website_family: 0,
    career_site: 0,
    generic: 0,
  };
  rows.forEach((row) => {
    counts[row.kind] += 1;
  });
  return counts;
}

export function countProfileOrigins(
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

export function profileKindEntries() {
  return Object.entries(profileKindLabels) as Array<[SourceProfileKind, string]>;
}

export function profileOriginEntries() {
  return Object.entries(originLabels) as Array<
    [SourceRegistryDocumentOrigin, string]
  >;
}

function profileSchemaSummary(profile: InstalledProfileWithDefinition) {
  const parts = [
    profile.definition.sourceConfigSchema ? "Profil-Schema" : null,
    profile.definition.detection ? "Detection" : null,
  ].filter(Boolean);

  const pathSchemaCount = profile.definition.accessPaths.filter(
    (accessPath) => accessPath.sourceConfigSchema,
  ).length;

  if (pathSchemaCount) {
    parts.push(
      `${pathSchemaCount} Pfad-Schema${pathSchemaCount === 1 ? "" : "s"}`,
    );
  }

  return parts.join(" · ") || "keine Zusatzblöcke";
}

function profileCapabilities(profile: InstalledProfileWithDefinition) {
  return unique(profile.definition.accessPaths.flatMap(accessPathCapabilities));
}

function profileSupportEvidenceKinds(profile: InstalledProfileWithDefinition) {
  return unique(
    profile.definition.support.evidence?.map((evidence) => evidence.kind) ?? [],
  );
}

function profileDetectionEvidenceKinds(profile: InstalledProfileWithDefinition) {
  return unique(
    profile.definition.detection?.evidence?.map((evidence) => evidence.kind) ?? [],
  );
}

function accessPathCapabilities(accessPath: ProfileAccessPathDefinition) {
  return [
    accessPath.discovery ? "discovery" : null,
    accessPath.detail ? "detail" : null,
  ].filter(Boolean) as string[];
}

function summarizeList(values: string[], emptyLabel: string) {
  if (!values.length) return emptyLabel;
  if (values.length <= 3) return values.join(", ");
  return `${values.slice(0, 3).join(", ")} +${values.length - 3}`;
}

function unique<T extends string>(values: T[]): T[] {
  return [...new Set(values)];
}
