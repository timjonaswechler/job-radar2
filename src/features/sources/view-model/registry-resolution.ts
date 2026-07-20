import { effectiveSourceConfigSchema } from "@/features/sources/shared/source-config-schema";
import type {
  JsonValue,
  ProfileAccessPathDefinition,
  RegistrySource,
  RegistrySourceProfile,
  SourceOwnedSelectedAccessPath,
  SupportLevel,
} from "@/lib/api/sources";

export type SourceResolution = {
  profile: RegistrySourceProfile | null;
  profileAccessPath: ProfileAccessPathDefinition | null;
  baseProfileAccessPath: ProfileAccessPathDefinition | null;
  sourceOwnedAccessPath: SourceOwnedSelectedAccessPath | null;
  effectiveSourceConfigSchema: JsonValue;
  supportLevel: SupportLevel | null;
  capabilities: string[];
};

export function resolveSource(
  source: RegistrySource,
  profilesByKey: Map<string, RegistrySourceProfile>,
): SourceResolution {
  const selectedAccessPath = source.document.selectedAccessPath;

  if (selectedAccessPath.type === "source_owned_access_path") {
    return {
      profile: null,
      profileAccessPath: null,
      baseProfileAccessPath: null,
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
  const baseProfileAccessPath =
    profile?.document.accessPaths.find(
      (accessPath) => accessPath.key === selectedAccessPath.pathKey,
    ) ?? null;
  const effectiveProfile = source.effectiveProfile ?? profile?.document;
  const profileAccessPath =
    effectiveProfile?.accessPaths.find(
      (accessPath) => accessPath.key === selectedAccessPath.pathKey,
    ) ?? null;

  return {
    profile,
    profileAccessPath,
    baseProfileAccessPath,
    sourceOwnedAccessPath: null,
    effectiveSourceConfigSchema: effectiveSourceConfigSchema(
      profile?.document.sourceConfigSchema,
      profileAccessPath?.sourceConfigSchema,
    ),
    supportLevel: profile?.document.support.level ?? null,
    capabilities: profileAccessPath ? accessPathCapabilities(profileAccessPath) : [],
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
