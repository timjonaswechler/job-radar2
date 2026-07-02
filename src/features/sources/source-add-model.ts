import { sourceConfigFromEntries, type SchemaMetadata, type SourceConfigEntry } from "@/features/sources/source-config-schema";
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
      schemaVersion: 1,
      key: form.key,
      name: form.name.trim(),
      status: form.status,
      sourceConfig: configResult.value,
      selectedAccessPath: {
        type: "profile",
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
