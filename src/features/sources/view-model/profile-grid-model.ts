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
  RegistrySourceProfile,
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
  path: string;
  searchText: string;
  profile: RegistrySourceProfile;
};

export type ProfileGridFilters = {
  searchQuery: string;
  kinds: SourceProfileKind[];
  origins: SourceRegistryDocumentOrigin[];
  diagnosticsOnly: boolean;
};

export function createProfileGridRows(
  profiles: RegistrySourceProfile[],
  diagnosticsByProfileKey: Map<string, StructuredDiagnostic[]>,
): ProfileGridRow[] {
  return profiles.map((profile) => {
    const diagnostics = uniqueDiagnostics([
      ...(diagnosticsByProfileKey.get(profile.document.key) ?? []),
      ...(profile.document.diagnostics ?? []),
      ...profile.document.accessPaths.flatMap(
        (accessPath) => accessPath.diagnostics ?? [],
      ),
    ]);
    const diagnosticSummary = classifyProfileRegistryRowHealth(diagnostics);
    const kindLabel = profileKindLabels[profile.document.kind];
    const supportLabel = supportLevelLabels[profile.document.support.level];
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
      profile.document.key,
      profile.document.name,
      kindLabel,
      profile.document.kind,
      supportLabel,
      profile.document.support.level,
      supportEvidenceSummary,
      supportEvidenceKinds.join(" "),
      detectionEvidenceSummary,
      detectionEvidenceKinds.join(" "),
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
      supportEvidenceKinds,
      supportEvidenceLabels,
      supportEvidenceSummary,
      detectionEvidenceKinds,
      detectionEvidenceLabels,
      detectionEvidenceSummary,
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

function profileSchemaSummary(profile: RegistrySourceProfile) {
  const parts = [
    profile.document.sourceConfigSchema ? "Profil-Schema" : null,
    profile.document.detection ? "Detection" : null,
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

function profileSupportEvidenceKinds(profile: RegistrySourceProfile) {
  return unique(
    profile.document.support.evidence?.map((evidence) => evidence.kind) ?? [],
  );
}

function profileDetectionEvidenceKinds(profile: RegistrySourceProfile) {
  return unique(
    profile.document.detection?.evidence?.map((evidence) => evidence.kind) ?? [],
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
