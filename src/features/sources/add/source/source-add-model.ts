import {
  configEntriesFromJsonObject,
  effectiveSourceConfigSchema,
  entriesWithSchemaHints,
  sourceConfigFromEntries,
  sourceConfigSchemaMetadata,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
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

export const emptySourceForm: SourceFormState = {
  name: "",
  key: "",
  status: "draft",
  profileKey: "",
  pathKey: "",
};

export type SourceFormState = {
  name: string;
  key: string;
  status: SourceStatus;
  profileKey: string;
  pathKey: string;
};

export type SourceBuildResult = {
  document: SourceDocument | null;
  errors: string[];
  configErrors: string[];
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

export type SourceAddDraftState = {
  form: SourceFormState;
  keyTouched: boolean;
  configEntries: SourceConfigEntry[];
  jsonPreviewOpen: boolean;
  saveAttempted: boolean;
};

export type SourceAddDraftDetectionResult = SourceAddDraftState & {
  appliedDetectedSource: boolean;
};

export function sourceFormAfterNameChange(
  form: SourceFormState,
  keyTouched: boolean,
  name: string,
): SourceFormState {
  return {
    ...form,
    name,
    key: keyTouched ? form.key : technicalKeyFromText(name),
  };
}

export function sourceFormAfterKeyChange(
  form: SourceFormState,
  key: string,
): SourceFormState {
  return { ...form, key: technicalKeyFromText(key) };
}

export function sourceAddDraftAfterProfileChange({
  profiles,
  form,
  configEntries,
  profileKey,
  createConfigEntryId = createEntryId,
}: {
  profiles: RegistrySourceProfile[];
  form: SourceFormState;
  configEntries: SourceConfigEntry[];
  profileKey: string;
  createConfigEntryId?: () => string;
}): Pick<SourceAddDraftState, "form" | "configEntries"> {
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

export function sourceAddDraftAfterAccessPathChange({
  selectedProfile,
  form,
  configEntries,
  pathKey,
  createConfigEntryId = createEntryId,
}: {
  selectedProfile: RegistrySourceProfile | null;
  form: SourceFormState;
  configEntries: SourceConfigEntry[];
  pathKey: string;
  createConfigEntryId?: () => string;
}): Pick<SourceAddDraftState, "form" | "configEntries"> {
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

export function sourceAddDraftAfterDetectedSource({
  profiles,
  detected,
  createConfigEntryId = createEntryId,
}: {
  profiles: RegistrySourceProfile[];
  detected: DetectedSourceLike;
  createConfigEntryId?: () => string;
}): SourceAddDraftState {
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
    jsonPreviewOpen: false,
    saveAttempted: false,
  };
}

export function sourceAddDraftAfterDetectionResult({
  draft,
  profiles,
  result,
  trimmedUrl,
  createConfigEntryId = createEntryId,
}: {
  draft: SourceAddDraftState;
  profiles: RegistrySourceProfile[];
  result: SourceProposalDetectionResult;
  trimmedUrl: string;
  createConfigEntryId?: () => string;
}): SourceAddDraftDetectionResult {
  if (result.status === "matched") {
    const detected = detectedSourceFromResult(result);
    if (detected) {
      return {
        ...sourceAddDraftAfterDetectedSource({
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

export function buildSourceDocument({
  form,
  configEntries,
  existingSourceKeys,
  selectedProfile,
  selectedAccessPath,
  schemaMetadata,
}: {
  form: SourceFormState;
  configEntries: SourceConfigEntry[];
  existingSourceKeys: Set<string>;
  selectedProfile: RegistrySourceProfile | null;
  selectedAccessPath: ProfileAccessPathDefinition | null;
  schemaMetadata: SchemaMetadata;
}): SourceBuildResult {
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
  errors.push(...configResult.errors);

  if (errors.length || !selectedProfile || !selectedAccessPath) {
    return { document: null, errors, configErrors: configResult.errors };
  }

  return {
    document: {
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
    },
    errors,
    configErrors: configResult.errors,
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

export function createEntryId() {
  return crypto.randomUUID();
}

export function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
