import { countLabel, ruleSummary, searchRequestTitle } from "@/features/search-requests/labels";
import {
  searchRequestStatusLabels,
  searchRunStatusLabels,
} from "@/features/search-requests/status";
import { selectedMissingSourceKeys } from "@/features/search-requests/model/source-options";
import type { SearchRequest, SearchRequestStatus } from "@/lib/api/search-requests";
import type { RegistrySource, SourceKey } from "@/lib/api/sources";

export type SearchRequestGroupId = "attention" | "active" | "draft" | "disabled";

export type SearchRequestTableRow = {
  id: number;
  title: string;
  status: SearchRequestStatus;
  statusLabel: string;
  includeSummary: string;
  excludeSummary: string;
  includeCount: number;
  excludeCount: number;
  sourceSummary: string;
  sourceCount: number;
  missingSourceKeys: SourceKey[];
  locationsSummary: string;
  radiusLabel: string;
  validationLabel: string;
  validationError: string | null;
  lastRunLabel: string;
  lastRunError: string | null;
  groupId: SearchRequestGroupId;
  groupLabel: string;
  groupRank: number;
  searchText: string;
  request: SearchRequest;
};

export type SearchRequestRowFilters = {
  searchQuery: string;
  statuses: SearchRequestStatus[];
  attentionOnly: boolean;
};

const groupLabels: Record<SearchRequestGroupId, string> = {
  attention: "Braucht Aufmerksamkeit",
  active: "Aktiv",
  draft: "Entwurf",
  disabled: "Deaktiviert",
};

const groupRanks: Record<SearchRequestGroupId, number> = {
  attention: 0,
  active: 1,
  draft: 2,
  disabled: 3,
};

export function createSearchRequestRows(
  requests: SearchRequest[],
  sources: RegistrySource[],
): SearchRequestTableRow[] {
  const sourcesByKey = new Map(
    sources.map((source) => [source.document.key, source.document.name]),
  );

  return requests.map((request) => {
    const missingSourceKeys = selectedMissingSourceKeys(request.sourceKeys, sources);
    const sourceLabels = request.sourceKeys.map(
      (sourceKey) => sourcesByKey.get(sourceKey) ?? sourceKey,
    );
    const groupId = searchRequestGroupId(request, missingSourceKeys);
    const statusLabel = searchRequestStatusLabels[request.status];
    const includeSummary = ruleSummary(request.includeRules, "Keine Include-Regeln");
    const excludeSummary = ruleSummary(request.excludeRules, "Keine Exclude-Regeln");
    const locationsSummary = request.locations.length
      ? request.locations.join(", ")
      : "Keine Orte";
    const radiusLabel = request.radiusKm === null ? "Kein Radius" : `${request.radiusKm} km`;
    const validationLabel = request.validationError ? "Validation Error" : "OK";
    const lastRunLabel = lastRunSummary(request);
    const sourceSummary = sourceLabels.length
      ? summarizeList(sourceLabels, "Keine Sources")
      : "Keine Sources";
    const searchText = [
      request.id,
      statusLabel,
      request.status,
      includeSummary,
      excludeSummary,
      sourceLabels.join(" "),
      missingSourceKeys.join(" "),
      locationsSummary,
      radiusLabel,
      request.validationError ?? "",
      request.lastRunStatus ? searchRunStatusLabels[request.lastRunStatus] : "",
      request.lastRunError ?? "",
    ]
      .join(" ")
      .toLocaleLowerCase("de");

    return {
      id: request.id,
      title: searchRequestTitle(request.id),
      status: request.status,
      statusLabel,
      includeSummary,
      excludeSummary,
      includeCount: request.includeRules.length,
      excludeCount: request.excludeRules.length,
      sourceSummary,
      sourceCount: request.sourceKeys.length,
      missingSourceKeys,
      locationsSummary,
      radiusLabel,
      validationLabel,
      validationError: request.validationError,
      lastRunLabel,
      lastRunError: request.lastRunError,
      groupId,
      groupLabel: groupLabels[groupId],
      groupRank: groupRanks[groupId],
      searchText,
      request,
    };
  });
}

export function filterSearchRequestRows(
  rows: SearchRequestTableRow[],
  filters: SearchRequestRowFilters,
): SearchRequestTableRow[] {
  const normalizedSearch = filters.searchQuery.trim().toLocaleLowerCase("de");

  return rows.filter(
    (row) =>
      (!normalizedSearch || row.searchText.includes(normalizedSearch)) &&
      (!filters.statuses.length || filters.statuses.includes(row.status)) &&
      (!filters.attentionOnly || row.groupId === "attention"),
  );
}

export function countSearchRequestStatuses(rows: SearchRequestTableRow[]) {
  const counts: Record<SearchRequestStatus, number> = {
    draft: 0,
    active: 0,
    disabled: 0,
    invalid: 0,
  };
  for (const row of rows) counts[row.status] += 1;
  return counts;
}

export function countAttentionRows(rows: SearchRequestTableRow[]) {
  return rows.filter((row) => row.groupId === "attention").length;
}

export function groupSearchRequestRows(rows: SearchRequestTableRow[]) {
  return [...rows]
    .sort((left, right) => left.groupRank - right.groupRank || left.id - right.id)
    .reduce<Array<{ id: SearchRequestGroupId; label: string; rows: SearchRequestTableRow[] }>>(
      (groups, row) => {
        const existingGroup = groups.find((group) => group.id === row.groupId);
        if (existingGroup) {
          existingGroup.rows.push(row);
          return groups;
        }
        groups.push({ id: row.groupId, label: row.groupLabel, rows: [row] });
        return groups;
      },
      [],
    );
}

function searchRequestGroupId(
  request: SearchRequest,
  missingSourceKeys: SourceKey[],
): SearchRequestGroupId {
  if (
    request.status === "invalid" ||
    request.validationError ||
    missingSourceKeys.length > 0 ||
    request.lastRunStatus === "failed" ||
    request.lastRunError
  ) {
    return "attention";
  }

  if (request.status === "active") return "active";
  if (request.status === "disabled") return "disabled";
  return "draft";
}

function lastRunSummary(request: SearchRequest) {
  if (!request.lastRunAt && !request.lastRunStatus) return "Noch nicht gelaufen";
  const status = request.lastRunStatus
    ? searchRunStatusLabels[request.lastRunStatus]
    : "Status unbekannt";
  const date = request.lastRunAt ? formatDateTime(request.lastRunAt) : "Zeit unbekannt";
  return `${status} · ${date}`;
}

function formatDateTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("de", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function summarizeList(values: string[], emptyLabel: string) {
  if (!values.length) return emptyLabel;
  if (values.length <= 2) return values.join(", ");
  return `${values.slice(0, 2).join(", ")} +${values.length - 2}`;
}

export function sourceCountLabel(count: number) {
  return countLabel(count, "Source", "Sources");
}
