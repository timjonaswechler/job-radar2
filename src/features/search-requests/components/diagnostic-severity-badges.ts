import type { BadgeProps } from "@/components/reui/badge";
import type { StructuredDiagnostic } from "@/lib/api/sources";

export const diagnosticSeverityBadgeVariants: Record<
  StructuredDiagnostic["severity"],
  BadgeProps["variant"]
> = {
  info: "info-light",
  warning: "warning-light",
  error: "destructive-light",
};
