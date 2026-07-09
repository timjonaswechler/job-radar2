import type {
  JsonValue,
  RegistrySource,
  RegistrySourceProfile,
  SourceRegistryDocumentKind,
  SourceRegistryDocumentOrigin,
  StructuredDiagnostic,
} from "@/lib/api/sources";

export function diagnosticCountLabel(count: number) {
  return `${count} Diagnose${count === 1 ? "" : "n"}`;
}

export type DiagnosticIndex = {
  bySourceKey: Map<string, StructuredDiagnostic[]>;
  byProfileKey: Map<string, StructuredDiagnostic[]>;
  unassigned: StructuredDiagnostic[];
};

export function buildDiagnosticIndex(
  sources: RegistrySource[],
  profiles: RegistrySourceProfile[],
  diagnostics: StructuredDiagnostic[],
): DiagnosticIndex {
  const bySourceKey = new Map<string, StructuredDiagnostic[]>();
  const byProfileKey = new Map<string, StructuredDiagnostic[]>();
  const sourceKeys = new Set(sources.map((source) => source.document.key));
  const profileKeys = new Set(profiles.map((profile) => profile.document.key));
  const sourceKeyByPath = new Map(
    sources.map((source) => [source.path, source.document.key]),
  );
  const profileKeyByPath = new Map(
    profiles.map((profile) => [profile.path, profile.document.key]),
  );
  const unassigned: StructuredDiagnostic[] = [];

  for (const diagnostic of diagnostics) {
    let attached = false;
    const details = diagnosticDetails(diagnostic);
    const documentKind = diagnosticDocumentKind(diagnostic);
    const diagnosticPath = diagnosticDocumentPath(diagnostic);
    const detailSourceKey = stringValue(details.sourceKey);
    const detailProfileKey = stringValue(details.sourceProfileKey);
    const detailKey = stringValue(details.key);

    const sourceKey = detailSourceKey ?? (documentKind === "source" ? detailKey : null);
    if (sourceKey && sourceKeys.has(sourceKey)) {
      pushDiagnostic(bySourceKey, sourceKey, diagnostic);
      attached = true;
    }

    const profileKey =
      detailProfileKey ?? (documentKind === "source_profile" ? detailKey : null);
    if (profileKey && profileKeys.has(profileKey)) {
      pushDiagnostic(byProfileKey, profileKey, diagnostic);
      attached = true;
    }

    if (!attached && diagnosticPath) {
      const sourcePathKey = sourceKeyByPath.get(diagnosticPath);
      if (sourcePathKey) {
        pushDiagnostic(bySourceKey, sourcePathKey, diagnostic);
        attached = true;
      }
      const profilePathKey = profileKeyByPath.get(diagnosticPath);
      if (profilePathKey) {
        pushDiagnostic(byProfileKey, profilePathKey, diagnostic);
        attached = true;
      }
    }

    if (!attached) {
      unassigned.push(diagnostic);
    }
  }

  return { bySourceKey, byProfileKey, unassigned };
}


export function diagnosticDocumentKind(
  diagnostic: StructuredDiagnostic,
): SourceRegistryDocumentKind | null {
  const value = stringValue(diagnosticDetails(diagnostic).documentKind);
  return value === "source" || value === "source_profile" ? value : null;
}


export function diagnosticDocumentOrigin(
  diagnostic: StructuredDiagnostic,
): SourceRegistryDocumentOrigin | null {
  const value = stringValue(diagnosticDetails(diagnostic).origin);
  return value === "built_in" || value === "custom" ? value : null;
}


export function diagnosticDocumentPath(diagnostic: StructuredDiagnostic): string | null {
  return stringValue(diagnosticDetails(diagnostic).path);
}


export function diagnosticDocumentKey(diagnostic: StructuredDiagnostic): string | null {
  const details = diagnosticDetails(diagnostic);
  return (
    stringValue(details.key) ??
    stringValue(details.sourceKey) ??
    stringValue(details.sourceProfileKey)
  );
}


export function isSourceDependencyDiagnostic(diagnostic: StructuredDiagnostic) {
  return [
    "missing_source_profile",
    "missing_profile",
    "missing_access_path",
    "recommended_access_path_not_found",
  ].includes(diagnostic.code);
}


export function pushDiagnostic(
  target: Map<string, StructuredDiagnostic[]>,
  key: string,
  diagnostic: StructuredDiagnostic,
) {
  const diagnostics = target.get(key);
  if (diagnostics) {
    diagnostics.push(diagnostic);
  } else {
    target.set(key, [diagnostic]);
  }
}


export function uniqueDiagnostics(diagnostics: StructuredDiagnostic[]) {
  const seen = new Set<string>();
  return diagnostics.filter((diagnostic) => {
    const key = [
      diagnostic.category,
      diagnostic.code,
      diagnostic.path,
      diagnostic.message,
      diagnostic.strategyKey ?? "",
    ].join("\0");
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}


export function diagnosticDetails(diagnostic: StructuredDiagnostic) {
  return isJsonObject(diagnostic.details) ? diagnostic.details : {};
}


export function isJsonObject(value: JsonValue | undefined): value is { [key: string]: JsonValue } {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

export function stringValue(value: JsonValue | undefined): string | null {
  return typeof value === "string" && value ? value : null;
}
