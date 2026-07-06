import type { SearchRule, SearchRuleKind, SearchRuleTarget } from "@/lib/api/search-requests";

export const searchRuleTargetLabels: Record<SearchRuleTarget, string> = {
  title: "Titel",
};

export const searchRuleKindLabels: Record<SearchRuleKind, string> = {
  text: "Text",
  regex: "Regex",
};

export function searchRequestTitle(id: number) {
  return `Search Request #${id}`;
}

export function ruleSummary(rules: SearchRule[], emptyLabel: string) {
  if (!rules.length) return emptyLabel;
  const values = rules.map((rule) => rule.value);
  if (values.length <= 2) return values.join(", ");
  return `${values.slice(0, 2).join(", ")} +${values.length - 2}`;
}

export function countLabel(count: number, singular: string, plural: string) {
  return `${count} ${count === 1 ? singular : plural}`;
}
