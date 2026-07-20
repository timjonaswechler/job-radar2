import {
  configEntriesFromJsonObject,
  entriesWithSchemaHints,
  sourceConfigFromEntries,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { directSourceSpecializationFromText } from "@/features/sources/source-form/direct-source-specialization";
import type { RegistrySource, SourceDocument, SourceStatus } from "@/lib/api/sources";

export type SourceEditDraftState = {
  name: string;
  status: SourceStatus;
  configEntries: SourceConfigEntry[];
  directSourceSpecializationText: string;
};

export type SourceEditBuildResult = {
  document: SourceDocument | null;
  errors: string[];
  configErrors: string[];
  specializationErrors: string[];
};

export type SourceEditDraftSnapshot = {
  name: string;
  status: SourceStatus;
  configEntries: Array<{ key: string; value: string }>;
  directSourceSpecializationText: string;
};

export function sourceEditDraftSnapshot({
  name,
  status,
  configEntries,
  directSourceSpecializationText,
}: SourceEditDraftState): SourceEditDraftSnapshot {
  return {
    name,
    status,
    configEntries: configEntries.map(({ key, value }) => ({ key, value })),
    directSourceSpecializationText,
  };
}

export function isSourceEditDraftDirty(
  draft: SourceEditDraftState,
  baseline: SourceEditDraftState,
) {
  return (
    JSON.stringify(sourceEditDraftSnapshot(draft)) !==
    JSON.stringify(sourceEditDraftSnapshot(baseline))
  );
}

export function sourceEditDraftFromSource({
  source,
  schemaMetadata,
  createConfigEntryId,
}: {
  source: RegistrySource;
  schemaMetadata: SchemaMetadata;
  createConfigEntryId: () => string;
}): SourceEditDraftState {
  return {
    name: source.document.name,
    status: source.document.status,
    configEntries: entriesWithSchemaHints(
      configEntriesFromJsonObject(
        source.document.sourceConfig,
        createConfigEntryId,
      ).map((entry) => ({
        ...entry,
        locked: schemaMetadata.requiredKeys.has(entry.key) || undefined,
      })),
      schemaMetadata,
      createConfigEntryId,
    ),
    directSourceSpecializationText:
      source.document.accessPaths === undefined
        ? ""
        : JSON.stringify(source.document.accessPaths, null, 2),
  };
}

export function buildUpdatedSourceDocument({
  source,
  name,
  status,
  configEntries,
  directSourceSpecializationText,
  schemaMetadata,
}: {
  source: RegistrySource;
  name: string;
  status: SourceStatus;
  configEntries: SourceConfigEntry[];
  directSourceSpecializationText: string;
  schemaMetadata: SchemaMetadata;
}): SourceEditBuildResult {
  const errors: string[] = [];

  if (!name.trim()) errors.push("Name fehlt.");

  const configResult = sourceConfigFromEntries(configEntries, schemaMetadata);
  const specializationResult = directSourceSpecializationFromText(directSourceSpecializationText);
  errors.push(...configResult.errors, ...specializationResult.errors);

  if (errors.length) {
    return {
      document: null,
      errors,
      configErrors: configResult.errors,
      specializationErrors: specializationResult.errors,
    };
  }

  const document: SourceDocument = {
    ...source.document,
    name: name.trim(),
    status,
    sourceConfig: configResult.value,
  };

  if (specializationResult.value === null) {
    delete document.accessPaths;
  } else {
    document.accessPaths = specializationResult.value;
  }

  return {
    document,
    errors,
    configErrors: configResult.errors,
    specializationErrors: specializationResult.errors,
  };
}
