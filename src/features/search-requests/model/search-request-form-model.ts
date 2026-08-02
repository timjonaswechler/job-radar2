import type {
  CreateSearchRequestInput,
  SearchRequest,
  SearchRequestStatus,
  SearchRule,
  SearchRuleKind,
  UpdateSearchRequestInput,
} from "@/lib/api/search-requests";
import type { SourceKey } from "@/lib/api/sources";

export type SearchRuleDraft = {
  target: "title";
  kind: SearchRuleKind;
  value: string;
};

export type SearchRequestFormState = {
  status: SearchRequestStatus;
  includeRules: SearchRuleDraft[];
  excludeRules: SearchRuleDraft[];
  locationsText: string;
  radiusKmText: string;
  sourceKeys: SourceKey[];
};

export type SearchRequestFormBuildResult = {
  input: CreateSearchRequestInput | UpdateSearchRequestInput | null;
  errors: string[];
  warnings: string[];
};

export const emptySearchRequestForm: SearchRequestFormState = createEmptySearchRequestForm();

export function createEmptySearchRequestForm(
  defaultSearchRadiusKm: number | null = null,
): SearchRequestFormState {
  return {
    status: "draft",
    includeRules: [emptySearchRuleDraft()],
    excludeRules: [],
    locationsText: "",
    radiusKmText: formatDefaultRadius(defaultSearchRadiusKm),
    sourceKeys: [],
  };
}

export function emptySearchRuleDraft(): SearchRuleDraft {
  return { target: "title", kind: "text", value: "" };
}

export function searchRequestFormFromRequest(
  request: SearchRequest,
): SearchRequestFormState {
  return {
    status: request.status,
    includeRules: request.includeRules.length
      ? request.includeRules.map(searchRuleDraftFromRule)
      : [emptySearchRuleDraft()],
    excludeRules: request.excludeRules.map(searchRuleDraftFromRule),
    locationsText: request.locations.join("\n"),
    radiusKmText: request.radiusKm === null ? "" : String(request.radiusKm),
    sourceKeys: request.sourceKeys,
  };
}

export function buildSearchRequestInput(
  form: SearchRequestFormState,
): SearchRequestFormBuildResult {
  const errors: string[] = [];
  const warnings: string[] = [];
  const includeRules = normalizeRuleDrafts(form.includeRules);
  const excludeRules = normalizeRuleDrafts(form.excludeRules);
  const locations = form.locationsText
    .split(/\r?\n|,/)
    .map((location) => location.trim())
    .filter(Boolean);
  const radiusKm = parseRadiusKm(form.radiusKmText, errors);
  const sourceKeys = trimmed(form.sourceKeys);
  if (new Set(sourceKeys).size !== sourceKeys.length) {
    const message = "Ein Source Key ist doppelt ausgewählt; entferne die Source und wähle sie einmal neu aus.";
    warnings.push(message);
    if (form.status === "active") errors.push(message);
  }

  if (form.status === "active") {
    if (!includeRules.length) {
      errors.push("Aktive Search Requests brauchen mindestens eine Include-Regel.");
    }
    if (!sourceKeys.length) {
      errors.push("Aktive Search Requests brauchen mindestens eine Source.");
    }
  }

  if (errors.length) return { input: null, errors, warnings };

  return {
    input: {
      status: form.status,
      includeRules,
      excludeRules,
      locations,
      radiusKm,
      sourceKeys,
    },
    errors: [],
    warnings,
  };
}

function searchRuleDraftFromRule(rule: SearchRule): SearchRuleDraft {
  return { target: "title", kind: rule.kind, value: rule.value };
}

function normalizeRuleDrafts(drafts: SearchRuleDraft[]): SearchRule[] {
  return drafts.flatMap((draft) => {
    const value = draft.value.trim();
    if (!value) return [];

    return [{ target: "title", kind: draft.kind, value }];
  });
}

function formatDefaultRadius(defaultSearchRadiusKm: number | null) {
  return defaultSearchRadiusKm === null ? "" : String(defaultSearchRadiusKm);
}

function parseRadiusKm(value: string, errors: string[]) {
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (!/^\d+$/.test(trimmed)) {
    errors.push("Radius muss eine ganze Zahl ab 0 sein.");
    return null;
  }
  const radiusKm = Number(trimmed);
  if (!Number.isSafeInteger(radiusKm)) {
    errors.push(`Radius darf höchstens ${Number.MAX_SAFE_INTEGER} sein.`);
    return null;
  }
  return radiusKm;
}

function trimmed(values: string[]) {
  return values.map((value) => value.trim()).filter(Boolean);
}
