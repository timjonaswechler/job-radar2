import type { BadgeProps } from "@/components/reui/badge";
import type { SearchRequestStatus, SearchRunStatus } from "@/lib/api/search-requests";

export const searchRequestStatusLabels: Record<SearchRequestStatus, string> = {
  draft: "Entwurf",
  active: "Aktiv",
  disabled: "Deaktiviert",
  invalid: "Ungültig",
};

export const searchRequestStatusBadgeVariants: Record<
  SearchRequestStatus,
  BadgeProps["variant"]
> = {
  draft: "primary-outline",
  active: "success-light",
  disabled: "invert-light",
  invalid: "destructive-light",
};

export const searchRequestStatusOptions = Object.entries(
  searchRequestStatusLabels,
).map(([value, label]) => ({ value: value as SearchRequestStatus, label }));

export const searchRunStatusLabels: Record<SearchRunStatus, string> = {
  completed: "Abgeschlossen",
  completed_with_errors: "Mit Fehlern abgeschlossen",
  failed: "Fehlgeschlagen",
  cancelled: "Abgebrochen",
};

export const searchRunStatusBadgeVariants: Record<
  SearchRunStatus,
  BadgeProps["variant"]
> = {
  completed: "success-light",
  completed_with_errors: "warning-light",
  failed: "destructive-light",
  cancelled: "invert-light",
};
