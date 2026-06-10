import type { BadgeProps } from "@/components/reui/badge";
import type { SourceStatus } from "@/lib/api/sources";

export const sourceStatusLabels: Record<SourceStatus, string> = {
  draft: "Entwurf",
  active: "Aktiv",
  disabled: "Deaktiviert",
  invalid: "Ungültig",
};

export const sourceStatusBadgeVariants: Record<
  SourceStatus,
  BadgeProps["variant"]
> = {
  draft: "primary-outline",
  active: "success-light",
  disabled: "invert-light",
  invalid: "destructive-light",
};

export const sourceStatusOptions = Object.entries(sourceStatusLabels).map(
  ([value, label]) => ({ value: value as SourceStatus, label }),
);
