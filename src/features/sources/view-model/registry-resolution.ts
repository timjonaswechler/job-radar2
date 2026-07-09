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

function accessPathCapabilities(
  accessPath: ProfileAccessPathDefinition | SourceOwnedSelectedAccessPath,
) {
  return [
    accessPath.postingDiscovery ? "postingDiscovery" : null,
    accessPath.postingDetail ? "postingDetail" : null,
  ].filter(Boolean) as string[];
}
