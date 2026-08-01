import {
  configEntriesFromJsonObject,
  entriesWithSchemaHints,
  sourceConfigFromEntries,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { directSourceSpecializationFromText } from "@/features/sources/source-form/direct-source-specialization";
import type { InstalledSource, ReviseSourceDefinition } from "@/lib/api/sources";

export type SourceEditDraftState = {
  name: string;
  configEntries: SourceConfigEntry[];
  directSourceSpecializationText: string;
};

export type SourceEditBuildResult = {
  document: ReviseSourceDefinition | null;
  errors: string[];
  configErrors: string[];
  specializationErrors: string[];
};

export type SourceEditDraftSnapshot = {
  name: string;
  configEntries: Array<{ key: string; value: string }>;
  directSourceSpecializationText: string;
};

export function sourceEditDraftSnapshot({
  name,
  configEntries,
  directSourceSpecializationText,
}: SourceEditDraftState): SourceEditDraftSnapshot {
  return {
    name,
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
  source: InstalledSource;
  schemaMetadata: SchemaMetadata;
  createConfigEntryId: () => string;
}): SourceEditDraftState {
  return {
    name: source.document.name,
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
  configEntries,
  directSourceSpecializationText,
  schemaMetadata,
}: {
  source: InstalledSource;
  name: string;
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

  const document: ReviseSourceDefinition = {
    key: source.document.key,
    name: name.trim(),
    sourceConfig: configResult.value,
    selectedAccessPath: source.document.selectedAccessPath,
    sourceSupport: source.document.sourceSupport,
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
