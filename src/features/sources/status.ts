import type { BadgeProps } from "@/components/reui/badge";
import type { SourceStatus, ValidationStateKind } from "@/lib/api/sources";

export const sourceStatusLabels: Record<SourceStatus, string> = {
  draft: "Entwurf",
  active: "Aktiv",
  disabled: "Deaktiviert",
};

export const sourceStatusBadgeVariants: Record<
  SourceStatus,
  BadgeProps["variant"]
> = {
  draft: "primary-outline",
  active: "success-light",
  disabled: "invert-light",
};

export const sourceStatusOptions = Object.entries(sourceStatusLabels).map(
  ([value, label]) => ({ value: value as SourceStatus, label }),
);

export const validationStateBadgeVariants: Record<
  ValidationStateKind,
  BadgeProps["variant"]
> = {
  unknown: "secondary",
  valid: "success-light",
  invalid: "destructive-light",
};
