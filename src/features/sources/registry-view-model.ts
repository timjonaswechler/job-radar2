import { getAdapterDisplay } from "@/features/sources/adapter-metadata";
import { originLabels, profileKindLabels } from "@/features/sources/labels";
import { effectiveSourceConfigSchema } from "@/features/sources/source-config-schema";
import { sourceStatusLabels } from "@/features/sources/status";
import type {
  AdapterMetadata,
  JsonValue,
  ProfileAccessPathDefinition,
  RegistrySource,
  RegistrySourceProfile,
  SelectedAccessPath,
  SourceProfileKind,
  SourceRegistryDiagnostic,
  SourceRegistryDocumentOrigin,
  SourceStatus,
} from "@/lib/api/sources";

export type SourceRegistryInventory = {
  adapters: AdapterMetadata[];
  profiles: RegistrySourceProfile[];
  sources: RegistrySource[];
  diagnostics: SourceRegistryDiagnostic[];
};

export type DiagnosticIndex = {
  bySourceKey: Map<string, SourceRegistryDiagnostic[]>;
  byProfileKey: Map<string, SourceRegistryDiagnostic[]>;
  unassigned: SourceRegistryDiagnostic[];
};

export type SourceResolution = {
  adapterKey: string | null;
  adapter: AdapterMetadata | null;
  profile: RegistrySourceProfile | null;
  profileAccessPath: ProfileAccessPathDefinition | null;
  effectiveSourceConfigSchema: JsonValue;
};

export type SourceGridRow = {
  key: string;
  name: string;
  status: SourceStatus;
  statusLabel: string;
  origin: SourceRegistryDocumentOrigin;
  originLabel: string;
  accessPathLabel: string;
  adapterLabel: string;
  profileLabel: string;
  configSummary: string;
  diagnosticsCount: number;
  path: string;
  searchText: string;
  source: RegistrySource;
};

export type ProfileGridRow = {
  key: string;
  name: string;
  kind: SourceProfileKind;
  kindLabel: string;
  origin: SourceRegistryDocumentOrigin;
  originLabel: string;
  accessPathCount: number;
  adapterSummary: string;
  schemaSummary: string;
  diagnosticsCount: number;
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

export function buildDiagnosticIndex(
  sources: RegistrySource[],
  profiles: RegistrySourceProfile[],
  diagnostics: SourceRegistryDiagnostic[],
): DiagnosticIndex {
  const bySourceKey = new Map<string, SourceRegistryDiagnostic[]>();
  const byProfileKey = new Map<string, SourceRegistryDiagnostic[]>();
  const sourceKeys = new Set(sources.map((source) => source.document.key));
  const profileKeys = new Set(profiles.map((profile) => profile.document.key));
  const sourceKeyByPath = new Map(
    sources.map((source) => [source.path, source.document.key]),
  );
  const profileKeyByPath = new Map(
    profiles.map((profile) => [profile.path, profile.document.key]),
  );
  const unassigned: SourceRegistryDiagnostic[] = [];

  for (const diagnostic of diagnostics) {
    let attached = false;

    if (diagnostic.documentKind === "source") {
      const key = diagnostic.key ?? sourceKeyByPath.get(diagnostic.path);
      if (key && sourceKeys.has(key)) {
        pushDiagnostic(bySourceKey, key, diagnostic);
        attached = true;
      }
    }

    if (diagnostic.documentKind === "source_profile") {
      const key = diagnostic.key ?? profileKeyByPath.get(diagnostic.path);
      if (key && profileKeys.has(key)) {
        pushDiagnostic(byProfileKey, key, diagnostic);
        attached = true;
      }
    }

    if (!attached) {
      unassigned.push(diagnostic);
    }
  }

  return { bySourceKey, byProfileKey, unassigned };
}

export function createSourceGridRows(
  sources: RegistrySource[],
  profilesByKey: Map<string, RegistrySourceProfile>,
  adaptersByKey: Map<string, AdapterMetadata>,
  diagnosticsBySourceKey: Map<string, SourceRegistryDiagnostic[]>,
): SourceGridRow[] {
  return sources.map((source) => {
    const resolution = resolveSource(source, profilesByKey, adaptersByKey);
    const selectedAccessPath = source.document.selectedAccessPath;
    const adapterDisplay = resolution.adapterKey
      ? getAdapterDisplay(resolution.adapterKey, resolution.adapter)
      : null;
    const diagnosticsCount =
      diagnosticsBySourceKey.get(source.document.key)?.length ?? 0;
    const accessPathLabel = accessPathSummary(selectedAccessPath);
    const profileLabel =
      selectedAccessPath.type === "profile"
        ? `${selectedAccessPath.profileKey} / ${selectedAccessPath.pathKey}`
        : "source_specific";
    const adapterLabel = adapterDisplay?.label ?? "—";
    const configSummary = jsonObjectSummary(source.document.sourceConfig);
    const statusLabel = sourceStatusLabels[source.document.status];
    const originLabel = originLabels[source.origin];
    const searchText = [
      source.document.key,
      source.document.name,
      statusLabel,
      source.document.status,
      originLabel,
      source.origin,
      accessPathLabel,
      profileLabel,
      resolution.profile?.document.name ?? "",
      adapterLabel,
      resolution.adapterKey ?? "",
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
      origin: source.origin,
      originLabel,
      accessPathLabel,
      adapterLabel,
      profileLabel,
      configSummary,
      diagnosticsCount,
      path: source.path,
      searchText,
      source,
    };
  });
}

export function createProfileGridRows(
  profiles: RegistrySourceProfile[],
  adaptersByKey: Map<string, AdapterMetadata>,
  diagnosticsByProfileKey: Map<string, SourceRegistryDiagnostic[]>,
): ProfileGridRow[] {
  return profiles.map((profile) => {
    const adapterKeys = unique(
      profile.document.accessPaths.map((accessPath) => accessPath.adapterKey),
    );
    const adapterLabels = adapterKeys.map((adapterKey) => {
      const display = getAdapterDisplay(
        adapterKey,
        adaptersByKey.get(adapterKey),
      );
      return display.registered ? display.name : display.label;
    });
    const adapterSummary = summarizeList(adapterLabels, "Keine Adapter");
    const diagnosticsCount =
      diagnosticsByProfileKey.get(profile.document.key)?.length ?? 0;
    const kindLabel = profileKindLabels[profile.document.kind];
    const originLabel = originLabels[profile.origin];
    const schemaSummary = profileSchemaSummary(profile);
    const searchText = [
      profile.document.key,
      profile.document.name,
      kindLabel,
      profile.document.kind,
      originLabel,
      profile.origin,
      adapterSummary,
      schemaSummary,
      profile.path,
      profile.document.accessPaths
        .map((accessPath) => accessPath.key)
        .join(" "),
    ]
      .join(" ")
      .toLocaleLowerCase("de");

    return {
      key: profile.document.key,
      name: profile.document.name,
      kind: profile.document.kind,
      kindLabel,
      origin: profile.origin,
      originLabel,
      accessPathCount: profile.document.accessPaths.length,
      adapterSummary,
      schemaSummary,
      diagnosticsCount,
      path: profile.path,
      searchText,
      profile,
    };
  });
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
  adaptersByKey: Map<string, AdapterMetadata>,
): SourceResolution {
  const selectedAccessPath = source.document.selectedAccessPath;

  if (selectedAccessPath.type === "source_specific") {
    return {
      adapterKey: selectedAccessPath.adapterKey,
      adapter: adaptersByKey.get(selectedAccessPath.adapterKey) ?? null,
      profile: null,
      profileAccessPath: null,
      effectiveSourceConfigSchema: effectiveSourceConfigSchema(
        undefined,
        selectedAccessPath.sourceConfigSchema,
      ),
    };
  }

  const profile = profilesByKey.get(selectedAccessPath.profileKey) ?? null;
  const profileAccessPath =
    profile?.document.accessPaths.find(
      (accessPath) => accessPath.key === selectedAccessPath.pathKey,
    ) ?? null;
  const adapterKey = profileAccessPath?.adapterKey ?? null;

  return {
    adapterKey,
    adapter: adapterKey ? (adaptersByKey.get(adapterKey) ?? null) : null,
    profile,
    profileAccessPath,
    effectiveSourceConfigSchema: effectiveSourceConfigSchema(
      profile?.document.sourceConfigSchema,
      profileAccessPath?.sourceConfigSchema,
    ),
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
  return Object.entries(profileKindLabels) as Array<
    [SourceProfileKind, string]
  >;
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

function pushDiagnostic(
  target: Map<string, SourceRegistryDiagnostic[]>,
  key: string,
  diagnostic: SourceRegistryDiagnostic,
) {
  const diagnostics = target.get(key);
  if (diagnostics) {
    diagnostics.push(diagnostic);
  } else {
    target.set(key, [diagnostic]);
  }
}

function accessPathSummary(selectedAccessPath: SelectedAccessPath) {
  if (selectedAccessPath.type === "source_specific") {
    return `quellenspezifisch · ${selectedAccessPath.adapterKey}`;
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
    profile.document.detect ? "Detect" : null,
    profile.document.identity ? "Identity" : null,
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
    invalid: 0,
  };
}

function zeroProfileKindCounts(): Record<SourceProfileKind, number> {
  return {
    recruiting_system: 0,
    job_portal: 0,
    website_family: 0,
    generic: 0,
  };
}
