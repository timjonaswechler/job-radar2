import {
  configEntriesFromJsonObject,
  createSourceConfigEntryId,
  effectiveSourceConfigSchema,
  entriesWithSchemaHints,
  sourceConfigFromEntries,
  sourceConfigSchemaMetadata,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { sourceOverridesFromText } from "@/features/sources/source-form/source-overrides";
import type {
  JsonValue,
  ProfileAccessPathDefinition,
  RegistrySourceProfile,
  SourceDocument,
  SourceProposal,
  SourceProposalDetectionResult,
  SourceStatus,
} from "@/lib/api/sources";

export const sourceKeyPattern = /^[a-z0-9_]+$/;

export const emptySourceCreateForm: SourceCreateFormState = {
  name: "",
  key: "",
  status: "draft",
  profileKey: "",
  pathKey: "",
};

export type SourceCreateFormState = {
  name: string;
  key: string;
  status: SourceStatus;
  profileKey: string;
  pathKey: string;
};

export type SourceCreateBuildResult = {
  document: SourceDocument | null;
  errors: string[];
  configErrors: string[];
  overridesErrors: string[];
};

export type DetectedSourceLike = {
  profileKey: string;
  pathKey: string;
  key: string;
  name: string;
  sourceConfig: JsonValue;
};

export function detectedSourceFromResult(
  result: SourceProposalDetectionResult,
): DetectedSourceLike | null {
  return result.proposal ? detectedSourceFromProposal(result.proposal) : null;
}

export function detectedSourceFromProposal(
  proposal: SourceProposal,
): DetectedSourceLike | null {
  const key = proposal.keyCandidates[0];
  const name = proposal.nameCandidates[0];
  if (!proposal.profileKey || !proposal.recommendedAccessPathKey || !key || !name) {
    return null;
  }
  return {
    profileKey: proposal.profileKey,
    pathKey: proposal.recommendedAccessPathKey,
    key,
    name,
    sourceConfig: proposal.sourceConfig,
  };
}

export type SourceCreateDraftState = {
  form: SourceCreateFormState;
  keyTouched: boolean;
  configEntries: SourceConfigEntry[];
  sourceOverridesText: string;
  jsonPreviewOpen: boolean;
  saveAttempted: boolean;
};

export type SourceCreateDraftDetectionResult = SourceCreateDraftState & {
  appliedDetectedSource: boolean;
};

export type SourceCreateDraftInput = {
  url: string;
  form: SourceCreateFormState;
  configEntries: readonly SourceConfigEntry[];
  sourceOverridesText: string;
};

export type SourceCreateDraftSnapshot = {
  url: string;
  name: string;
  key: string;
  status: SourceStatus;
  profileKey: string;
  pathKey: string;
  configEntries: Array<{ key: string; value: string }>;
  sourceOverridesText: string;
};

const emptySourceCreateDraftSnapshot: SourceCreateDraftSnapshot = {
  url: "",
  name: "",
  key: "",
  status: "draft",
  profileKey: "",
  pathKey: "",
  configEntries: [],
  sourceOverridesText: "",
};

export function sourceCreateDraftSnapshot({
  url,
  form,
  configEntries,
  sourceOverridesText,
}: SourceCreateDraftInput): SourceCreateDraftSnapshot {
  return {
    url,
    name: form.name,
    key: form.key,
    status: form.status,
    profileKey: form.profileKey,
    pathKey: form.pathKey,
    configEntries: configEntries.map(({ key, value }) => ({ key, value })),
    sourceOverridesText,
  };
}

export function isSourceCreateDraftDirty(draft: SourceCreateDraftInput) {
  return (
    JSON.stringify(sourceCreateDraftSnapshot(draft)) !==
    JSON.stringify(emptySourceCreateDraftSnapshot)
  );
}

export function sourceCreateFormAfterNameChange(
  form: SourceCreateFormState,
  keyTouched: boolean,
  name: string,
): SourceCreateFormState {
  return {
    ...form,
    name,
    key: keyTouched ? form.key : technicalKeyFromText(name),
  };
}

export function sourceCreateFormAfterKeyChange(
  form: SourceCreateFormState,
  key: string,
): SourceCreateFormState {
  return { ...form, key: technicalKeyFromText(key) };
}

export function sourceCreateDraftAfterProfileChange({
  profiles,
  form,
  configEntries,
  profileKey,
  createConfigEntryId = createSourceConfigEntryId,
}: {
  profiles: RegistrySourceProfile[];
  form: SourceCreateFormState;
  configEntries: SourceConfigEntry[];
  profileKey: string;
  createConfigEntryId?: () => string;
}): Pick<SourceCreateDraftState, "form" | "configEntries"> {
  const nextProfile =
    profiles.find((profile) => profile.document.key === profileKey) ?? null;
  const nextPath = nextProfile?.document.accessPaths[0];
  const nextSchema = effectiveSourceConfigSchema(
    nextProfile?.document.sourceConfigSchema,
    nextPath?.sourceConfigSchema,
  );
  const nextMetadata = sourceConfigSchemaMetadata(nextSchema);

  return {
    form: {
      ...form,
      profileKey,
      pathKey: nextPath?.key ?? "",
    },
    configEntries: entriesWithSchemaHints(
      configEntries,
      nextMetadata,
      createConfigEntryId,
    ),
  };
}

export function sourceCreateDraftAfterAccessPathChange({
  selectedProfile,
  form,
  configEntries,
  pathKey,
  createConfigEntryId = createSourceConfigEntryId,
}: {
  selectedProfile: RegistrySourceProfile | null;
  form: SourceCreateFormState;
  configEntries: SourceConfigEntry[];
  pathKey: string;
  createConfigEntryId?: () => string;
}): Pick<SourceCreateDraftState, "form" | "configEntries"> {
  const nextPath = selectedProfile?.document.accessPaths.find(
    (accessPath) => accessPath.key === pathKey,
  );
  const nextSchema = effectiveSourceConfigSchema(
    selectedProfile?.document.sourceConfigSchema,
    nextPath?.sourceConfigSchema,
  );
  const nextMetadata = sourceConfigSchemaMetadata(nextSchema);

  return {
    form: { ...form, pathKey },
    configEntries: entriesWithSchemaHints(
      configEntries,
      nextMetadata,
      createConfigEntryId,
    ),
  };
}

export function sourceCreateDraftAfterDetectedSource({
  profiles,
  detected,
  createConfigEntryId = createSourceConfigEntryId,
}: {
  profiles: RegistrySourceProfile[];
  detected: DetectedSourceLike;
  createConfigEntryId?: () => string;
}): SourceCreateDraftState {
  const nextProfile =
    profiles.find((profile) => profile.document.key === detected.profileKey) ??
    null;
  const nextPath =
    nextProfile?.document.accessPaths.find(
      (accessPath) => accessPath.key === detected.pathKey,
    ) ?? null;
  const nextSchema = effectiveSourceConfigSchema(
    nextProfile?.document.sourceConfigSchema,
    nextPath?.sourceConfigSchema,
  );
  const nextMetadata = sourceConfigSchemaMetadata(nextSchema);

  return {
    form: {
      name: detected.name,
      key: detected.key,
      status: "draft",
      profileKey: detected.profileKey,
      pathKey: detected.pathKey,
    },
    keyTouched: false,
    configEntries: entriesWithSchemaHints(
      configEntriesFromJsonObject(detected.sourceConfig, createConfigEntryId),
      nextMetadata,
      createConfigEntryId,
    ),
    sourceOverridesText: "",
    jsonPreviewOpen: false,
    saveAttempted: false,
  };
}

export function sourceCreateDraftAfterDetectionResult({
  draft,
  profiles,
  result,
  trimmedUrl,
  createConfigEntryId = createSourceConfigEntryId,
}: {
  draft: SourceCreateDraftState;
  profiles: RegistrySourceProfile[];
  result: SourceProposalDetectionResult;
  trimmedUrl: string;
  createConfigEntryId?: () => string;
}): SourceCreateDraftDetectionResult {
  if (result.status === "matched") {
    const detected = detectedSourceFromResult(result);
    if (detected) {
      return {
        ...sourceCreateDraftAfterDetectedSource({
          profiles,
          detected,
          createConfigEntryId,
        }),
        appliedDetectedSource: true,
      };
    }
  }

  if (result.status === "unsupported") {
    return {
      ...draft,
      configEntries: draft.configEntries.some((entry) => entry.key === "startUrl")
        ? draft.configEntries
        : [
            ...draft.configEntries,
            {
              id: createConfigEntryId(),
              key: "startUrl",
              value: trimmedUrl,
            },
          ],
      appliedDetectedSource: false,
    };
  }

  return { ...draft, appliedDetectedSource: false };
}

export function buildCreatedSourceDocument({
  form,
  configEntries,
  sourceOverridesText = "",
  existingSourceKeys,
  selectedProfile,
  selectedAccessPath,
  schemaMetadata,
}: {
  form: SourceCreateFormState;
  configEntries: SourceConfigEntry[];
  sourceOverridesText?: string;
  existingSourceKeys: Set<string>;
  selectedProfile: RegistrySourceProfile | null;
  selectedAccessPath: ProfileAccessPathDefinition | null;
  schemaMetadata: SchemaMetadata;
}): SourceCreateBuildResult {
  const errors: string[] = [];

  if (!form.name.trim()) errors.push("Name fehlt.");
  if (!form.key.trim()) errors.push("Key fehlt.");
  if (form.key && !sourceKeyPattern.test(form.key)) {
    errors.push("Key darf nur Kleinbuchstaben, Zahlen und Unterstriche enthalten.");
  }
  if (form.key && existingSourceKeys.has(form.key)) {
    errors.push(`Eine Quelle mit dem Key „${form.key}“ existiert bereits.`);
  }
  if (!selectedProfile) errors.push("Quellenprofil fehlt.");
  if (!selectedAccessPath) errors.push("Zugriffspfad fehlt.");

  const configResult = sourceConfigFromEntries(configEntries, schemaMetadata);
  const overridesResult = sourceOverridesFromText(sourceOverridesText);
  errors.push(...configResult.errors, ...overridesResult.errors);

  if (errors.length || !selectedProfile || !selectedAccessPath) {
    return {
      document: null,
      errors,
      configErrors: configResult.errors,
      overridesErrors: overridesResult.errors,
    };
  }

  const document: SourceDocument = {
    schemaVersion: 2,
    key: form.key,
    name: form.name.trim(),
    status: form.status,
    sourceConfig: configResult.value,
    selectedAccessPath: {
      type: "profile_access_path",
      profileKey: selectedProfile.document.key,
      pathKey: selectedAccessPath.key,
    },
  };

  if (overridesResult.value !== null) {
    document.sourceOverrides = overridesResult.value;
  }

  return {
    document,
    errors,
    configErrors: configResult.errors,
    overridesErrors: overridesResult.errors,
  };
}

export function technicalKeyFromText(value: string) {
  return value
    .trim()
    .toLocaleLowerCase("de")
    .replace(/ä/g, "ae")
    .replace(/ö/g, "oe")
    .replace(/ü/g, "ue")
    .replace(/ß/g, "ss")
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .replace(/_+/g, "_");
}

export function accessPathDisplayName(accessPath: ProfileAccessPathDefinition) {
  return accessPath.name ? `${accessPath.name} · ${accessPath.key}` : accessPath.key;
}
