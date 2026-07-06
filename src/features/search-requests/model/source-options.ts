import { validationStateLabels } from "@/features/sources/labels";
import { sourceStatusLabels } from "@/features/sources/status";
import type { RegistrySource, SourceKey } from "@/lib/api/sources";

export type SearchRequestSourceOption = {
  key: SourceKey;
  name: string;
  statusLabel: string;
  validationStateLabel: string;
  canExecute: boolean;
  missing: boolean;
  searchText: string;
};

export function createSearchRequestSourceOptions(
  sources: RegistrySource[],
  selectedSourceKeys: SourceKey[] = [],
): SearchRequestSourceOption[] {
  const optionsByKey = new Map<SourceKey, SearchRequestSourceOption>();

  for (const source of sources) {
    const key = source.document.key;
    const statusLabel = sourceStatusLabels[source.document.status];
    const validationStateLabel = validationStateLabels[source.validationState.state];
    const option: SearchRequestSourceOption = {
      key,
      name: source.document.name,
      statusLabel,
      validationStateLabel,
      canExecute: source.validationState.canExecute,
      missing: false,
      searchText: [
        key,
        source.document.name,
        source.document.status,
        statusLabel,
        source.validationState.state,
        validationStateLabel,
      ]
        .join(" ")
        .toLocaleLowerCase("de"),
    };
    optionsByKey.set(key, option);
  }

  for (const sourceKey of selectedSourceKeys) {
    if (optionsByKey.has(sourceKey)) continue;
    optionsByKey.set(sourceKey, {
      key: sourceKey,
      name: sourceKey,
      statusLabel: "Nicht in Registry",
      validationStateLabel: "Fehlt",
      canExecute: false,
      missing: true,
      searchText: sourceKey.toLocaleLowerCase("de"),
    });
  }

  return [...optionsByKey.values()].sort((left, right) => {
    if (left.missing !== right.missing) return left.missing ? -1 : 1;
    return left.name.localeCompare(right.name, "de");
  });
}

export function selectedMissingSourceKeys(
  sourceKeys: SourceKey[],
  sources: RegistrySource[],
): SourceKey[] {
  const knownKeys = new Set(sources.map((source) => source.document.key));
  return sourceKeys.filter((sourceKey) => !knownKeys.has(sourceKey));
}
