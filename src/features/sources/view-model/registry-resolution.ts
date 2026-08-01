import { effectiveSourceConfigSchema } from "@/features/sources/shared/source-config-schema";
import type {
  JsonValue,
  ProfileAccessPathDefinition,
  InstalledSource,
  InstalledProfileWithDefinition,
  SourceOwnedSelectedAccessPath,
  SupportLevel,
} from "@/lib/api/sources";

export type SourceResolution = {
  profile: InstalledProfileWithDefinition | null;
  baseProfileAccessPath: ProfileAccessPathDefinition | null;
  sourceOwnedAccessPath: SourceOwnedSelectedAccessPath | null;
  resolvedAccessPathName: string | null;
  effectiveSourceConfigSchema: JsonValue;
  supportLevel: SupportLevel | null;
  capabilities: string[];
};

export function resolveSource(
  source: InstalledSource,
  profilesByKey: Map<string, InstalledProfileWithDefinition>,
): SourceResolution {
  const selectedAccessPath = source.document.selectedAccessPath;

  if (selectedAccessPath.type === "source_owned_access_path") {
    return {
      profile: null,
      baseProfileAccessPath: null,
      sourceOwnedAccessPath: selectedAccessPath,
      resolvedAccessPathName: source.resolved?.accessPathName ?? null,
      effectiveSourceConfigSchema: effectiveSourceConfigSchema(
        source.resolved?.profileSourceConfigSchema,
        source.resolved?.accessPathSourceConfigSchema ??
          selectedAccessPath.sourceConfigSchema,
      ),
      supportLevel:
        source.resolved?.support?.level ??
        source.document.sourceSupport?.level ??
        null,
      capabilities:
        source.resolved?.capabilities ??
        accessPathCapabilities(selectedAccessPath),
    };
  }

  const profile = profilesByKey.get(selectedAccessPath.profileKey) ?? null;
  const baseProfileAccessPath =
    profile?.definition.accessPaths.find(
      (accessPath) => accessPath.key === selectedAccessPath.pathKey,
    ) ?? null;
  return {
    profile,
    baseProfileAccessPath,
    sourceOwnedAccessPath: null,
    resolvedAccessPathName: source.resolved?.accessPathName ?? null,
    effectiveSourceConfigSchema: effectiveSourceConfigSchema(
      source.resolved?.profileSourceConfigSchema ??
        profile?.definition.sourceConfigSchema,
      source.resolved?.accessPathSourceConfigSchema ??
        baseProfileAccessPath?.sourceConfigSchema,
    ),
    supportLevel:
      source.resolved?.support?.level ??
      profile?.definition.support.level ??
      null,
    capabilities:
      source.resolved?.capabilities ??
      (source.validationState.canCompile && baseProfileAccessPath
        ? accessPathCapabilities(baseProfileAccessPath)
        : []),
  };
}

function accessPathCapabilities(
  accessPath: ProfileAccessPathDefinition | SourceOwnedSelectedAccessPath,
) {
  return [
    accessPath.discovery ? "discovery" : null,
    accessPath.detail ? "detail" : null,
  ].filter(Boolean) as string[];
}
