import {
  configEntriesFromJsonObject,
  entriesWithSchemaHints,
  sourceConfigFromEntries,
  type SchemaMetadata,
  type SourceConfigEntry,
} from "@/features/sources/shared/source-config-schema";
import { sourceOverridesFromText } from "@/features/sources/source-form/source-overrides";
import type { RegistrySource, SourceDocument, SourceStatus } from "@/lib/api/sources";

export type SourceEditDraftState = {
  name: string;
  status: SourceStatus;
  configEntries: SourceConfigEntry[];
  sourceOverridesText: string;
};

export type SourceEditBuildResult = {
  document: SourceDocument | null;
  errors: string[];
  configErrors: string[];
  overridesErrors: string[];
};

export type SourceEditDraftSnapshot = {
  name: string;
  status: SourceStatus;
  configEntries: Array<{ key: string; value: string }>;
  sourceOverridesText: string;
};

export function sourceEditDraftSnapshot({
  name,
  status,
  configEntries,
  sourceOverridesText,
}: SourceEditDraftState): SourceEditDraftSnapshot {
  return {
    name,
    status,
    configEntries: configEntries.map(({ key, value }) => ({ key, value })),
    sourceOverridesText,
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
    sourceOverridesText:
      source.document.sourceOverrides === undefined
        ? ""
        : JSON.stringify(source.document.sourceOverrides, null, 2),
  };
}

export function buildUpdatedSourceDocument({
  source,
  name,
  status,
  configEntries,
  sourceOverridesText,
  schemaMetadata,
}: {
  source: RegistrySource;
  name: string;
  status: SourceStatus;
  configEntries: SourceConfigEntry[];
  sourceOverridesText: string;
  schemaMetadata: SchemaMetadata;
}): SourceEditBuildResult {
  const errors: string[] = [];

  if (!name.trim()) errors.push("Name fehlt.");

  const configResult = sourceConfigFromEntries(configEntries, schemaMetadata);
  const overridesResult = sourceOverridesFromText(sourceOverridesText);
  errors.push(...configResult.errors, ...overridesResult.errors);

  if (errors.length) {
    return {
      document: null,
      errors,
      configErrors: configResult.errors,
      overridesErrors: overridesResult.errors,
    };
  }

  const document: SourceDocument = {
    ...source.document,
    name: name.trim(),
    status,
    sourceConfig: configResult.value,
  };

  if (overridesResult.value === null) {
    delete document.sourceOverrides;
  } else {
    document.sourceOverrides = overridesResult.value;
  }

  return {
    document,
    errors,
    configErrors: configResult.errors,
    overridesErrors: overridesResult.errors,
  };
}
